//! The text "Edit locally" works on (PLAN §9b): what the person selected in the pane, or else the
//! word under the pointer. The window hands it to the host-file editor, which resolves it against
//! the pane's folder on the host.

use temor_core::viewer::{candidates, token_at};
use temor_pane::text::CellPos;

use super::Session;
use super::pointer::screen_cell;

impl Session {
    /// The selected text, one line of it, if there is any.
    fn selected_line(&self) -> Option<String> {
        let selection = self.selection?;
        let text = self
            .pane
            .with_term(|term| selection.text(|row| term.row_text(row).unwrap_or_default()));
        let line = text.lines().map(str::trim).find(|line| !line.is_empty())?;
        Some(line.to_owned())
    }

    /// The word under the pointer, as the path it most likely names.
    fn word_under_pointer(&self) -> Option<String> {
        let at = self.pointer_over?;
        let fit = self.placed()?;
        let hit = fit.cell_at(at);
        let top = self.viewport_top().ok()?;
        let CellPos { row, col } = screen_cell(top, hit.pos.row.0, hit.pos.col.0);
        let token = token_at(&self.line(row).ok()?, col)?;
        candidates(&token).into_iter().next()
    }

    /// What to edit: the selection, else the word under the pointer.
    pub fn text_to_edit(&self) -> Option<String> {
        self.selected_line().or_else(|| self.word_under_pointer())
    }
}
