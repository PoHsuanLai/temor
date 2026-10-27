//! The look of a pane that is already open, changed by the settings (PLAN §14): its font, its
//! cursor's shape and its blinking. The rest of the settings are read when a pane starts.

use temor_pane::settings::{CursorBlinking, CursorStyle};
use temor_render::FontSet;

use super::Session;
use crate::error::UiError;

impl Session {
    /// Draws the grid with `fonts` from now on. The cell has a new size, so the grid is fitted to
    /// the pane again, and the program is told when the number of rows or columns changed.
    pub fn set_fonts(&mut self, fonts: FontSet) -> Result<(), UiError> {
        self.painter.set_fonts(fonts);
        let (pixels, scale) = (self.pixels, self.scale);
        self.layout(pixels, scale).map(|_| ())
    }

    /// Makes `style` the cursor shape the terminal falls back to.
    pub fn set_cursor_style(&mut self, style: CursorStyle) -> Result<(), UiError> {
        self.pane.with_term(|term| term.set_default_cursor(style))?;
        Ok(())
    }

    /// Whether the cursor blinks, from the next paint.
    pub fn set_cursor_blink(&mut self, blink: CursorBlinking) {
        self.blink.setting = blink;
        self.wake_cursor();
    }
}
