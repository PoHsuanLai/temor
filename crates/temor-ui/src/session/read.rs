//! What the window and the tests read from a session: the pane, the grid, the cursor, the
//! screen's text, the find bar and the paste waiting for an answer.

use temor_core::terminal::keymap::Keymap;
use temor_core::terminal::search::Finder;
use temor_render::GridFit;

use super::Session;
use super::clip::PendingPaste;
use crate::error::UiError;
use crate::pane::PaneHandle;

impl Session {
    /// The pane's engine handle, for the window's painting loop and tests.
    pub fn pane(&self) -> &PaneHandle {
        &self.pane
    }

    /// The grid as last fitted to the pane (`None` before the first layout).
    pub fn fit(&self) -> Option<GridFit> {
        self.fit
    }

    /// The cursor's cell in pane-relative logical pixels `[x, y, width, height]`, when shown.
    pub fn cursor_logical(&self) -> Option<[f32; 4]> {
        let fit = self.fit?;
        let pos = self.painter.cursor_cell()?;
        let (x, y) = fit.cell_origin(pos);
        let factor = self.scale.factor();
        Some([
            x.0 as f32 / factor,
            y.0 as f32 / factor,
            fit.cell.width.0 as f32 / factor,
            fit.cell.height.0 as f32 / factor,
        ])
    }

    /// The screen's text, row by row, as the terminal holds it (for tests and the probe).
    pub fn screen_text(&self) -> String {
        self.pane.with_term(|term| {
            let rows = term.screen_rows().unwrap_or(0);
            (0..rows)
                .map(|row| {
                    term.row_text(row)
                        .map(|line| line.text())
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
    }

    /// The find bar's state, if it is open.
    pub fn finder(&self) -> Option<&Finder> {
        self.finder.as_ref()
    }

    /// A paste waiting for the person's confirmation.
    pub fn pending_paste(&self) -> Option<&PendingPaste> {
        self.pending.as_ref()
    }

    /// The keymap the pane routes keys with.
    pub fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    /// The viewport's first screen row (what the pane shows at the top).
    pub(super) fn viewport_top(&self) -> Result<u32, UiError> {
        Ok(self.pane.with_term(|term| term.viewport())?.0)
    }
}
