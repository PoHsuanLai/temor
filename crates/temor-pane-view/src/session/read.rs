//! What the window and the tests read from a session: the pane, the grid, the cursor, the
//! screen's text, the find bar and the paste waiting for an answer.

use chordkit::Keymap;
use temor_panes::pane::text::Finder;
use temor_panes::render::GridFit;

use super::Session;
use super::clip::PendingPaste;
use crate::error::ViewError;
use temor_panes::pane::PaneHandle;

impl Session {
    /// The pane's engine handle, for the window's painting loop and tests.
    pub fn pane(&self) -> &PaneHandle {
        &self.pane
    }

    /// The grid as last fitted to the pane and placed in it (`None` before the first layout):
    /// lowered as the last paint drew it, so pixels and cells map as they are shown.
    pub fn fit(&self) -> Option<GridFit> {
        self.placed().map(|placed| placed.fit)
    }

    /// The font the grid is drawn in.
    pub fn grid_font(&self) -> &crate::paint::GridFont {
        self.painter.font()
    }

    /// The pane's width and one cell's width in logical pixels (`None` before the first layout).
    pub fn grid_logical(&self) -> Option<(f32, f32)> {
        let fit = self.placed()?.fit;
        let factor = self.scale.factor();
        Some((
            fit.pane.width.0 as f32 / factor,
            fit.cell.width.0 as f32 / factor,
        ))
    }

    /// The cursor's cell in pane-relative logical pixels `[x, y, width, height]`, when shown.
    pub fn cursor_logical(&self) -> Option<[f32; 4]> {
        let placed = self.placed()?;
        let fit = placed.fit;
        let pos = self.painter.cursor_cell()?;
        let (x, _) = fit.cell_origin(pos);
        // Where the cursor's row is shown: below the room under any block above it.
        let y = placed.layout.row_top(pos.row);
        let factor = self.scale.factor();
        Some([
            x.0 as f32 / factor,
            y as f32 / factor,
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
                        .map(|line| line.text().trim_end().to_owned())
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
    pub(super) fn viewport_top(&self) -> Result<u32, ViewError> {
        Ok(self.pane.with_term(|term| term.viewport())?.0)
    }
}
