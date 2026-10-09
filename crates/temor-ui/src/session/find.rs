//! The find bar (PLAN §6, ⌘F): every screen row is searched, the current match is selected (so
//! ⌘C copies it and the grid draws it) and scrolled into view.

use temor_core::terminal::search::{Finder, Match, find_all};
use temor_core::terminal::selection::{Granularity, Selection, Shape};

use super::Session;
use crate::error::UiError;

impl Session {
    /// Searches the whole screen (scrollback included) for `query` and makes the first match
    /// current. An empty query has no matches.
    pub fn set_query(&mut self, query: &str) -> Result<(), UiError> {
        let matches = self.pane.with_term(|term| -> Result<Vec<Match>, UiError> {
            let rows = term.screen_rows()?;
            let mut lines = Vec::with_capacity(usize::try_from(rows).unwrap_or(0));
            for row in 0..rows {
                lines.push((row, term.row_text(row)?));
            }
            Ok(find_all(
                lines.iter().map(|(row, text)| (*row, text.as_str())),
                query,
            ))
        })?;
        self.finder = Some(Finder::new(query, matches));
        self.reveal_current()
    }

    /// Moves the find bar to the next (or previous) match.
    pub(super) fn step_find(&mut self, forward: bool) {
        if let Some(finder) = self.finder.as_mut() {
            match forward {
                true => finder.next(),
                false => finder.previous(),
            }
        }
        // A failed reveal leaves the match where it was; the bar still shows its position.
        let _ = self.reveal_current();
    }

    /// Selects the current match and scrolls the viewport to show it.
    fn reveal_current(&mut self) -> Result<(), UiError> {
        let Some(found) = self.finder.as_ref().and_then(|f| f.current().copied()) else {
            self.selection = None;
            return self.mirror_selection();
        };
        let line = self.line(found.row)?;
        let mut selection =
            Selection::begin(found.start(), &line, Granularity::Cells, Shape::Linear);
        selection.extend(found.end(), &line);
        self.selection = Some(selection);
        self.mirror_selection()?;
        let (top, len) = self.pane.with_term(|term| term.viewport())?;
        if found.row < top || found.row >= top.saturating_add(len) {
            self.pane.with_term(|term| {
                term.scroll(temor_vt::ViewportScroll::Row(temor_vt::RowCount(
                    u64::from(found.row),
                )))
            });
        }
        Ok(())
    }
}
