//! Scrollback search (PLAN §6, ⌘F): every match of a query in the screen rows, case-insensitive,
//! and a cursor that steps through them and wraps. Pure: the caller reads the rows' text.

use super::chord::fold;
use super::selection::CellPos;

/// One match: a row, the first column and its length in characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Match {
    pub row: u32,
    pub col: u16,
    pub len: u16,
}

impl Match {
    /// The first cell of the match.
    pub fn start(&self) -> CellPos {
        CellPos::new(self.row, self.col)
    }

    /// The last cell of the match (inclusive).
    pub fn end(&self) -> CellPos {
        CellPos::new(
            self.row,
            self.col.saturating_add(self.len.saturating_sub(1)),
        )
    }
}

/// Every match of `query` in `rows` (screen row, text), left to right, non-overlapping within a
/// row. An empty query matches nothing.
pub fn find_all<'a>(rows: impl IntoIterator<Item = (u32, &'a str)>, query: &str) -> Vec<Match> {
    let needle: Vec<char> = query.chars().map(fold).collect();
    if needle.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (row, text) in rows {
        let hay: Vec<char> = text.chars().map(fold).collect();
        let mut at = 0;
        while at + needle.len() <= hay.len() {
            if hay[at..at + needle.len()] == needle[..] {
                out.push(Match {
                    row,
                    col: u16::try_from(at).unwrap_or(u16::MAX),
                    len: u16::try_from(needle.len()).unwrap_or(u16::MAX),
                });
                at += needle.len();
            } else {
                at += 1;
            }
        }
    }
    out
}

/// The find bar's state: the query, its matches, and which one is current.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finder {
    query: String,
    matches: Vec<Match>,
    current: Option<usize>,
}

impl Finder {
    /// A finder over `matches` for `query`. The first match is current when there is one.
    pub fn new(query: &str, matches: Vec<Match>) -> Self {
        let current = (!matches.is_empty()).then_some(0);
        Self {
            query: query.to_owned(),
            matches,
            current,
        }
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn matches(&self) -> &[Match] {
        &self.matches
    }

    /// The match the bar points at.
    pub fn current(&self) -> Option<&Match> {
        self.current.and_then(|i| self.matches.get(i))
    }

    /// "3 of 12" style position: the 1-based index of the current match and the total.
    pub fn position(&self) -> Option<(usize, usize)> {
        self.current.map(|i| (i + 1, self.matches.len()))
    }

    /// Moves to the next match, wrapping from the last to the first.
    pub fn next(&mut self) {
        self.current = self.current.map(|i| (i + 1) % self.matches.len());
    }

    /// Moves to the previous match, wrapping from the first to the last.
    pub fn previous(&mut self) {
        let len = self.matches.len();
        self.current = self.current.map(|i| (i + len - 1) % len);
    }
}
