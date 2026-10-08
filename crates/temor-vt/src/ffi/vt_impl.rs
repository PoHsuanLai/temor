//! `Terminal` as a [`Vt`].

use super::format::Formatter;
use super::terminal::Terminal;
use crate::{
    CellPx, GridSize, ModeState, TerminalMode, TextFormat, TextSpan, ViewportScroll, Vt, VtError,
};

impl Vt for Terminal {
    fn write(&mut self, bytes: &[u8]) {
        Terminal::write(self, bytes);
    }

    fn resize(&mut self, size: GridSize, cell: CellPx) -> Result<(), VtError> {
        Terminal::resize(self, size, cell)
    }

    fn scroll_viewport(&mut self, to: ViewportScroll) {
        Terminal::scroll_viewport(self, to);
    }

    fn mode(&self, mode: TerminalMode) -> Result<ModeState, VtError> {
        Terminal::mode(self, mode)
    }

    fn title(&self) -> String {
        Terminal::title(self)
    }

    fn pwd(&self) -> String {
        Terminal::pwd(self)
    }

    fn plain_text(&self) -> Result<String, VtError> {
        Formatter::new(TextFormat::Plain).format(self, TextSpan::Screen)
    }
}
