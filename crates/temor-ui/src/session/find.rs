//! The find bar (PLAN §6, ⌘F): every screen row is searched, the current match is selected (so
//! ⌘C copies it and the grid draws it) and scrolled into view. The rows are read a chunk at a
//! time, so the terminal is never locked for the whole scrollback, and a query runs once the
//! person pauses typing (the clock is passed in).

use std::time::Duration;

use temor_pane::text::RowText;
use temor_pane::text::{Finder, Match, find_all, may_match};
use temor_pane::text::{Granularity, Selection, Shape};

use super::Session;
use super::model::PendingQuery;
use crate::error::UiError;

/// How many screen rows are read per lock.
const ROWS_PER_LOCK: u32 = 256;

/// How many rows are formatted as text at once to see whether any could match; only blocks that
/// could are read cell by cell, which costs about a hundred times more.
const ROWS_PER_GLANCE: u32 = 32;

/// How long the person must pause typing before the search runs.
pub const QUERY_DEBOUNCE: Duration = Duration::from_millis(150);

/// Which way the find bar moves to the next match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FindStep {
    Next,
    Previous,
}

impl Session {
    /// The query as the person has typed it (it may not have run yet).
    pub fn find_text(&self) -> &str {
        &self.find_text
    }

    /// The find bar's text changed at `now`: the search runs after [`QUERY_DEBOUNCE`] of quiet.
    pub fn queue_query(&mut self, query: &str, now: Duration) {
        self.find_text = query.to_owned();
        self.pending_query = Some(PendingQuery {
            text: query.to_owned(),
            at: now,
        });
    }

    /// Runs the queued query if the quiet period has passed by `now`. Returns whether it ran.
    pub fn run_due_query(&mut self, now: Duration) -> Result<bool, UiError> {
        let due = self
            .pending_query
            .as_ref()
            .is_some_and(|p| now.saturating_sub(p.at) >= QUERY_DEBOUNCE);
        if !due {
            return Ok(false);
        }
        let Some(pending) = self.pending_query.take() else {
            return Ok(false);
        };
        self.set_query(&pending.text)?;
        Ok(true)
    }

    /// Searches the whole screen (scrollback included) for `query` and makes the first match
    /// current. An empty query has no matches.
    pub fn set_query(&mut self, query: &str) -> Result<(), UiError> {
        let rows = self.pane.with_term(|term| term.screen_rows())?;
        let mut lines: Vec<(u32, RowText)> = Vec::with_capacity(usize::try_from(rows).unwrap_or(0));
        let mut first = 0;
        while first < rows {
            let end = first.saturating_add(ROWS_PER_LOCK).min(rows);
            let chunk = self.pane.with_term(|term| {
                let mut chunk = Vec::new();
                let mut block = first;
                while block < end {
                    let next = block.saturating_add(ROWS_PER_GLANCE).min(end);
                    if may_match(&term.rows_text(block..next)?, query) {
                        for row in block..next {
                            chunk.push((row, term.row_text(row)?));
                        }
                    }
                    block = next;
                }
                Ok::<_, temor_pane::PaneError>(chunk)
            })?;
            lines.extend(chunk);
            first = end;
        }
        let matches: Vec<Match> = find_all(lines.iter().map(|(row, line)| (*row, line)), query);
        self.find_text = query.to_owned();
        self.finder = Some(Finder::new(query, matches));
        self.reveal_current()
    }

    /// Moves the find bar to the next or previous match.
    pub(super) fn step_find(&mut self, step: FindStep) {
        if let Some(finder) = self.finder.as_mut() {
            match step {
                FindStep::Next => finder.next(),
                FindStep::Previous => finder.previous(),
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
