//! The pane's terminal: libghostty-vt's `Terminal` with its key and mouse encoders and the
//! renderer's frame capture. The I/O thread writes it under the pane's lock; the window reads
//! and encodes under the same lock (PLAN §4b).

use std::sync::mpsc::{Receiver, channel};

use crate::paste::PasteMode;
use crate::text::{RowCell, RowText, Span, Wrap};
use temor_render::{CapturedFrame, FrameCapture, Palette};
use temor_vt::{
    AnchorId, CellPx, CellWidth, CursorShape, EventSink, Formatter, GridCell, GridPoint, GridSize,
    KeyEncoder, KeyInput, LineCount, ModeState, MouseEncoder, MouseInput, MouseSurface, PointSpace,
    ReplySink, Snapshot, Terminal, TerminalConfig, TerminalMode, TextFormat, TextSelection,
    TextSpan, ViewportScroll, VtEvent,
};

use crate::error::PaneError;

/// What one write to the terminal produced: events for the window and replies for the PTY.
#[derive(Debug, Default)]
pub struct Output {
    pub events: Vec<VtEvent>,
    pub replies: Vec<Vec<u8>>,
}

/// The terminal and everything that reads or encodes it.
#[derive(Debug)]
pub struct Term {
    terminal: Terminal,
    keys: KeyEncoder,
    mouse: MouseEncoder,
    events: Receiver<VtEvent>,
    replies: Receiver<Vec<u8>>,
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
        Self::assemble(size, cell, scrollback, cursor, palette, Terminal::new)
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
        Ok(Self {
            terminal,
            keys: KeyEncoder::new()?,
            mouse: MouseEncoder::new()?,
            events,
            replies,
        })
    }

    /// The whole terminal encoded, to start a pane from later.
    pub fn snapshot(&self) -> Result<Snapshot, PaneError> {
        Ok(self.terminal.snapshot()?)
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
            wrap: if self.terminal.row_wraps(row)? {
                Wrap::Continues
            } else {
                Wrap::Ends
            },
        })
    }

    /// The viewport's place: its first screen row, and the rows it shows.
    pub fn viewport(&self) -> Result<(u32, u32), PaneError> {
        let bar = self.terminal.scrollbar()?;
        let clamp = |v: u64| u32::try_from(v).unwrap_or(u32::MAX);
        Ok((clamp(bar.offset.0), clamp(bar.len.0)))
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

    /// How many anchors are live.
    pub fn anchor_count(&self) -> usize {
        self.terminal.anchor_count()
    }

    /// Whether a full-screen program holds the alternate screen (no shell marks show there).
    pub fn on_alternate_screen(&self) -> bool {
        self.terminal.on_alternate_screen()
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
