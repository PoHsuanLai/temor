//! The seam other crates program against, so they can test on [`crate::FakeVt`] without Zig.

use crate::{CellPx, GridSize, ModeState, TerminalMode, ViewportScroll, VtError};

/// A terminal emulator: bytes in, events and replies out (through the sinks it was created
/// with), state to read back. `Terminal` is the real one; `FakeVt` (feature `testing`) is a small stand-in.
pub trait Vt: Send {
    /// Feeds program output. Events for what it contained go to the sink before this returns.
    fn write(&mut self, bytes: &[u8]);

    /// Changes the grid size, reflowing soft-wrapped lines. Like `write`, it may send events
    /// (the end of a render hold) and replies (in-band size reports).
    fn resize(&mut self, size: GridSize, cell: CellPx) -> Result<(), VtError>;

    /// Moves the viewport over the scrollback.
    fn scroll_viewport(&mut self, to: ViewportScroll);

    /// Whether `mode` is on.
    fn mode(&self, mode: TerminalMode) -> Result<ModeState, VtError>;

    /// The title the program set; empty when none.
    fn title(&self) -> String;

    /// The working directory URL the program reported; empty when none.
    fn pwd(&self) -> String;

    /// The plain text of everything kept (scrollback and screen), soft wraps joined and trailing
    /// blanks trimmed.
    fn plain_text(&self) -> Result<String, VtError>;
}
