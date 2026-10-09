//! The pane's terminal: libghostty-vt's `Terminal` with its key and mouse encoders and the
//! renderer's frame capture. The I/O thread writes it under the pane's lock; the window reads
//! and encodes under the same lock (PLAN §4b).

use std::sync::mpsc::{Receiver, channel};

use temor_core::terminal::paste::PasteMode;
use temor_core::terminal::selection::RowText;
use temor_render::{FrameCapture, Palette, PaneFrame};
use temor_vt::{
    CellPx, EventSink, Formatter, GridPoint, GridSize, KeyEncoder, KeyInput, LineCount, ModeState,
    MouseEncoder, MouseInput, MouseSurface, PointSpace, ReplySink, Terminal, TerminalConfig,
    TerminalMode, TextFormat, TextSelection, TextSpan, ViewportScroll, VtEvent,
};

use crate::error::UiError;

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
    capture: FrameCapture,
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
        palette: &Palette,
    ) -> Result<Self, UiError> {
        let (event_tx, events) = channel();
        let (reply_tx, replies) = channel();
        let mut config = TerminalConfig::new(size);
        config.cell_px = cell;
        config.scrollback = scrollback;
        let sink: Box<dyn EventSink> = Box::new(event_tx);
        let reply: Box<dyn ReplySink> = Box::new(reply_tx);
        let mut terminal = Terminal::new(config, sink, reply)?;
        palette.apply(&mut terminal)?;
        Ok(Self {
            terminal,
            keys: KeyEncoder::new()?,
            mouse: MouseEncoder::new()?,
            capture: FrameCapture::new()?,
            events,
            replies,
        })
    }

    /// Feeds program output and collects what it caused.
    pub fn write(&mut self, bytes: &[u8]) -> Output {
        self.terminal.write(bytes);
        self.collect()
    }

    /// Changes the grid. Reflow events and in-band replies come back like a write's.
    pub fn resize(&mut self, size: GridSize, cell: CellPx) -> Result<Output, UiError> {
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
    pub fn encode_key(&mut self, key: &KeyInput) -> Result<Vec<u8>, UiError> {
        self.keys.sync(&self.terminal);
        Ok(self.keys.encode(key)?)
    }

    /// The bytes for a mouse event on `surface`; empty when the program does not track the
    /// mouse for it.
    pub fn encode_mouse(
        &mut self,
        input: &MouseInput,
        surface: MouseSurface,
    ) -> Result<Vec<u8>, UiError> {
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

    pub fn set_selection(&mut self, selection: Option<TextSelection>) -> Result<(), UiError> {
        Ok(self.terminal.set_selection(selection)?)
    }

    /// The text between two screen points, as the terminal formats it.
    pub fn text(&self, start: GridPoint, end: GridPoint) -> Result<String, UiError> {
        Ok(Formatter::new(TextFormat::Plain)
            .format(&self.terminal, TextSpan::Range { start, end })?)
    }

    /// The text of one screen row, from its first to its last column, and whether the row is
    /// soft-wrapped into the next.
    pub fn row_text(&self, row: u32) -> Result<RowText, UiError> {
        let last = self.terminal.size().cols.0.saturating_sub(1);
        let text = self.text(
            GridPoint::new(PointSpace::Screen, 0, row),
            GridPoint::new(PointSpace::Screen, last, row),
        )?;
        Ok(RowText {
            text,
            wraps: self.terminal.row_wraps(row)?,
        })
    }

    /// The viewport's place: its first screen row, and the rows it shows.
    pub fn viewport(&self) -> Result<(u32, u32), UiError> {
        let bar = self.terminal.scrollbar()?;
        let clamp = |v: u64| u32::try_from(v).unwrap_or(u32::MAX);
        Ok((clamp(bar.offset.0), clamp(bar.len.0)))
    }

    /// How many screen rows exist, scrollback included.
    pub fn screen_rows(&self) -> Result<u32, UiError> {
        Ok(u32::try_from(self.terminal.scrollbar()?.total.0).unwrap_or(u32::MAX))
    }

    /// Draws what changed into `frame`, under the caller's lock.
    pub fn capture(&mut self, frame: &mut PaneFrame) -> Result<(), UiError> {
        let captured = self.capture.begin(&mut self.terminal)?;
        captured.finish(frame)?;
        Ok(())
    }

    /// The title the program set.
    pub fn title(&self) -> String {
        self.terminal.title()
    }

    /// The hyperlink (OSC 8) under a grid point, if any.
    pub fn hyperlink_at(&self, at: GridPoint) -> Option<String> {
        self.terminal.hyperlink_at(at).ok().flatten()
    }

    pub fn set_palette(&mut self, palette: &Palette) -> Result<(), UiError> {
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
