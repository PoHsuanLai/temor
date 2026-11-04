//! The pane's terminal: libghostty-vt's `Terminal` with its key and mouse encoders and the
//! renderer's frame capture. The I/O thread writes it under the pane's lock; the window reads
//! and encodes under the same lock (PLAN §4b).

use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};

use super::clipboard::ClipboardMirror;
use super::hold::{HoldCapture, Shared};
use crate::paste::PasteMode;
use crate::text::{RowCell, RowText, Span, Wrap};
use temor_render::{CapturedFrame, FrameCapture, Palette};
use temor_vt::{
    AnchorId, CellPx, CellWidth, CursorShape, EventSink, Formatter, GridCell, GridPoint, GridSize,
    KeyEncoder, KeyInput, KittyKeyFlags, LineCount, ModeState, MouseEncoder, MouseInput,
    MouseSurface, PointSpace, ReplySink, Snapshot, Terminal, TerminalConfig, TerminalMode,
    TextFormat, TextSelection, TextSpan, ViewportScroll, VtEvent,
};

use crate::error::PaneError;
use crate::settings::CursorStyle;
use crate::shell::cursor_shape;

/// What one write to the terminal produced: events for the window and replies for the PTY.
#[derive(Debug, Default)]
pub struct Output {
    pub events: Vec<VtEvent>,
    pub replies: Vec<Vec<u8>>,
}

/// The terminal and everything that reads or encodes it.
#[derive(Debug)]
pub struct Term {
    pub(super) terminal: Terminal,
    keys: KeyEncoder,
    mouse: MouseEncoder,
    events: Receiver<VtEvent>,
    replies: Receiver<Vec<u8>>,
    clipboard: ClipboardMirror,
    pub(super) hold: Shared,
}

impl Term {
    /// A blank terminal of `size` cells of `cell` pixels, keeping `scrollback` lines, painted
    /// with `palette` (PLAN §5: applied before the first frame).
    pub fn new(
        size: GridSize,
        cell: CellPx,
        scrollback: LineCount,
        cursor: CursorShape,
        palette: &Palette,
    ) -> Result<Self, PaneError> {
        let mut term = Self::assemble(size, cell, scrollback, cursor, palette, Terminal::new)?;
        // The defaults apply at a cursor-style reset: the first prompt has the chosen shape and
        // blinks, as Ghostty's does, until a program says otherwise.
        term.terminal.write(b"\x1b[0 q");
        Ok(term)
    }

    /// A terminal restored from `snapshot` (its screen, scrollback, modes and cursor), with the
    /// same settings as [`Term::new`]. The snapshot's size wins over the grid until the window
    /// lays the pane out.
    pub fn restored(
        snapshot: &Snapshot,
        cell: CellPx,
        scrollback: LineCount,
        cursor: CursorShape,
        palette: &Palette,
    ) -> Result<Self, PaneError> {
        let size = GridSize::new(80, 24);
        Self::assemble(
            size,
            cell,
            scrollback,
            cursor,
            palette,
            |config, sink, reply| Terminal::restore(snapshot, config, sink, reply),
        )
    }

    fn assemble(
        size: GridSize,
        cell: CellPx,
        scrollback: LineCount,
        cursor: CursorShape,
        palette: &Palette,
        make: impl FnOnce(
            TerminalConfig,
            Box<dyn EventSink>,
            Box<dyn ReplySink>,
        ) -> Result<Terminal, temor_vt::VtError>,
    ) -> Result<Self, PaneError> {
        let (event_tx, events) = channel();
        let (reply_tx, replies) = channel();
        let mut config = TerminalConfig::new(size);
        config.cell_px = cell;
        config.scrollback = scrollback;
        let sink: Box<dyn EventSink> = Box::new(event_tx);
        let reply: Box<dyn ReplySink> = Box::new(reply_tx);
        let mut terminal = make(config, sink, reply)?;
        palette.apply(&mut terminal)?;
        terminal.set_default_cursor_shape(Some(cursor))?;
        terminal.set_default_cursor_blink(Some(true))?;
        let clipboard = ClipboardMirror::default();
        terminal.set_clipboard_source(Box::new(clipboard.clone()));
        let hold = Shared::default();
        terminal.set_hold_hook(Box::new(HoldCapture::new(Arc::clone(&hold))));
        let mut keys = KeyEncoder::new()?;
        keys.set_option_as_alt(crate::platform::OPTION_AS_ALT);
        Ok(Self {
            terminal,
            keys,
            mouse: MouseEncoder::new()?,
            events,
            replies,
            clipboard,
            hold,
        })
    }

    /// The copy of the clipboard that answers the program's reads (OSC 52), and whether it may.
    pub fn clipboard(&self) -> ClipboardMirror {
        self.clipboard.clone()
    }

    /// The whole terminal encoded, to start a pane from later.
    pub fn snapshot(&self) -> Result<Snapshot, PaneError> {
        Ok(self.terminal.snapshot()?)
    }

    /// Takes the prompt left open at the bottom off a restored screen, for a new program that
    /// draws its own (see `Terminal::take_open_prompt_off`).
    pub fn take_open_prompt_off(&mut self) {
        self.terminal.take_open_prompt_off();
    }

    /// Feeds program output and collects what it caused.
    pub fn write(&mut self, bytes: &[u8]) -> Output {
        self.terminal.write(bytes);
        self.collect()
    }

    /// Changes the grid. Reflow events and in-band replies come back like a write's.
    pub fn resize(&mut self, size: GridSize, cell: CellPx) -> Result<Output, PaneError> {
        self.terminal.resize(size, cell)?;
        Ok(self.collect())
    }

    /// The grid as the terminal holds it.
    pub fn size(&self) -> GridSize {
        self.terminal.size()
    }

    pub fn scroll(&mut self, to: ViewportScroll) {
        self.terminal.scroll_viewport(to);
    }

    /// The bytes for a key, after the program's kitty and cursor-key modes (PLAN §6).
    pub fn encode_key(&mut self, key: &KeyInput) -> Result<Vec<u8>, PaneError> {
        self.keys.sync(&self.terminal);
        Ok(self.keys.encode(key)?)
    }

    /// The kitty keyboard flags the program pushed.
    pub fn kitty_keyboard_flags(&self) -> KittyKeyFlags {
        self.terminal.kitty_keyboard_flags()
    }

    /// The bytes for a mouse event on `surface`; empty when the program does not track the
    /// mouse for it.
    pub fn encode_mouse(
        &mut self,
        input: &MouseInput,
        surface: MouseSurface,
    ) -> Result<Vec<u8>, PaneError> {
        self.mouse.sync(&self.terminal);
        self.mouse.set_surface(surface);
        Ok(self.mouse.encode(input)?)
    }

    /// Whether the program asked for any mouse reporting.
    pub fn mouse_tracking(&self) -> bool {
        [
            TerminalMode::MouseX10,
            TerminalMode::MouseNormal,
            TerminalMode::MouseButton,
            TerminalMode::MouseAny,
        ]
        .into_iter()
        .any(|mode| self.terminal.mode(mode) == Ok(ModeState::Set))
    }

    /// How a paste must be framed right now.
    pub fn paste_mode(&self) -> PasteMode {
        match self.terminal.mode(TerminalMode::BracketedPaste) {
            Ok(ModeState::Set) => PasteMode::Bracketed,
            _ => PasteMode::Plain,
        }
    }

    pub fn set_selection(&mut self, selection: Option<TextSelection>) -> Result<(), PaneError> {
        Ok(self.terminal.set_selection(selection)?)
    }

    /// The text between two screen points, as the terminal formats it.
    pub fn text(&self, start: GridPoint, end: GridPoint) -> Result<String, PaneError> {
        Ok(Formatter::new(TextFormat::Plain)
            .format(&self.terminal, TextSpan::Range { start, end })?)
    }

    /// Screen rows `rows` as the terminal formats them, one formatter call: far cheaper than
    /// reading them cell by cell (`search_bench`), but without columns.
    pub fn rows_text(&self, rows: std::ops::Range<u32>) -> Result<String, PaneError> {
        if rows.is_empty() {
            return Ok(String::new());
        }
        let last_col = self.terminal.size().cols.0.saturating_sub(1);
        self.text(
            GridPoint::new(PointSpace::Screen, 0, rows.start),
            GridPoint::new(PointSpace::Screen, last_col, rows.end - 1),
        )
    }

    /// One screen row, cell by cell, and whether it continues on the next (soft wrap).
    pub fn row_text(&self, row: u32) -> Result<RowText, PaneError> {
        let cells = self
            .terminal
            .row_cells(row)?
            .into_iter()
            .map(row_cell)
            .collect();
        Ok(RowText {
            cells,
            wrap: match self.terminal.row_wraps(row)? {
                temor_vt::Wrap::Continues => Wrap::Continues,
                _ => Wrap::Ends,
            },
        })
    }

    /// Screen rows `rows`, styled cell by cell, as a frame would draw them: a finished block's
    /// output, wherever it is now.
    pub fn styled_rows(
        &self,
        rows: std::ops::Range<u32>,
    ) -> Result<Vec<Vec<temor_vt::StyledCell>>, PaneError> {
        Ok(self.terminal.styled_rows(rows)?)
    }

    /// The viewport's place: its first screen row, and the rows it shows.
    pub fn viewport(&self) -> Result<(u32, u32), PaneError> {
        let bar = self.terminal.scrollbar()?;
        let clamp = |v: u64| u32::try_from(v).unwrap_or(u32::MAX);
        Ok((clamp(bar.offset.0), clamp(bar.len.0)))
    }

    /// The cursor's row in the viewport as the terminal has it now, not as the last frame drew it.
    /// `None` when the cursor is scrolled out of the viewport or the terminal will not say.
    pub fn cursor_viewport_row(&self) -> Option<u16> {
        let active_row = i64::from(self.terminal.cursor_row()?);
        let (top, rows) = self.viewport().ok()?;
        let total = i64::from(self.screen_rows().ok()?);
        let active_rows = i64::from(self.terminal.size().rows.0);
        let row = total - active_rows + active_row - i64::from(top);
        u16::try_from(row).ok().filter(|row| u32::from(*row) < rows)
    }

    /// How many screen rows exist, scrollback included.
    pub fn screen_rows(&self) -> Result<u32, PaneError> {
        Ok(u32::try_from(self.terminal.scrollbar()?.total.0).unwrap_or(u32::MAX))
    }

    /// Phase one of a frame, under the caller's lock: libghostty's update and the kitty
    /// placements. The returned frame is finished after the lock is released (PLAN §4b).
    pub fn begin_frame<'c>(
        &mut self,
        capture: &'c mut FrameCapture,
    ) -> Result<CapturedFrame<'c>, PaneError> {
        Ok(capture.begin(&mut self.terminal)?)
    }

    /// The title the program set.
    pub fn title(&self) -> String {
        self.terminal.title()
    }

    /// Where an anchor is now, in `space`; `None` once the scrollback has dropped it.
    pub fn anchor_point(&self, id: AnchorId, space: PointSpace) -> Option<GridPoint> {
        self.terminal.anchor_point(id, space)
    }

    /// Anchors the cursor's cell now (a command's end, for example).
    pub fn anchor_cursor(&mut self) -> Option<AnchorId> {
        self.terminal.anchor_cursor()
    }

    /// Anchors the cell at `at`.
    pub fn anchor_at(&mut self, at: GridPoint) -> Option<AnchorId> {
        self.terminal.anchor_at(at)
    }

    /// Where each kind of shell-marked text starts, top of the scrollback down (see
    /// `Terminal::semantic_starts`). Empty when the terminal will not say.
    pub fn semantic_starts(&self) -> Vec<(GridPoint, temor_vt::SemanticMark)> {
        self.terminal.semantic_starts().unwrap_or_default()
    }

    /// How many anchors are live.
    pub fn anchor_count(&self) -> usize {
        self.terminal.anchor_count()
    }

    /// Whether a full-screen program holds the alternate screen (no shell marks show there).
    pub fn on_alternate_screen(&self) -> bool {
        self.terminal.on_alternate_screen()
    }

    /// Numbers this terminal's anchors after `earlier`'s, for a terminal that replaces it: an id
    /// from the old terminal then names nothing here.
    pub(super) fn number_anchors_after(&mut self, earlier: &Term) {
        self.terminal.number_anchors_after(&earlier.terminal);
    }

    /// Frees an anchor the window no longer needs.
    pub fn release_anchor(&mut self, id: AnchorId) {
        self.terminal.release_anchor(id);
    }

    /// The hyperlink (OSC 8) under a grid point, if any.
    pub fn hyperlink_at(&self, at: GridPoint) -> Option<String> {
        self.terminal.hyperlink_at(at).ok().flatten()
    }

    pub fn set_palette(&mut self, palette: &Palette) -> Result<(), PaneError> {
        palette.apply(&mut self.terminal)?;
        Ok(())
    }

    /// Makes `style` the cursor shape the terminal falls back to: what a program that has set no
    /// shape of its own, or resets it, gets (a program's own choice stays until it resets).
    pub fn set_default_cursor(&mut self, style: CursorStyle) -> Result<(), PaneError> {
        self.terminal
            .set_default_cursor_shape(Some(cursor_shape(style)))?;
        Ok(())
    }

    fn collect(&mut self) -> Output {
        Output {
            events: self.events.try_iter().collect(),
            replies: self.replies.try_iter().collect(),
        }
    }
}

/// A terminal cell as the pane reads it: text and span.
fn row_cell(cell: GridCell) -> RowCell {
    let span = match cell.width {
        CellWidth::Wide => Span::Double,
        CellWidth::SpacerTail | CellWidth::SpacerHead => Span::Spacer,
        CellWidth::Narrow => Span::Single,
    };
    RowCell {
        text: cell.text,
        span,
    }
}
