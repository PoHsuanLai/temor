//! A point over the block list to the screen cell it shows (PLAN: block list): the list says
//! where each stretch of it lies and which screen rows it draws, and a point lands on a row, or,
//! between rows (a card's head, its foot, the gap after it), at the start of the rows that follow
//! or the end of those before.

use std::ops::Range;

use temor_core::vocab::ids::BlockId;

/// Which rows a stretch with no rows of its own goes with, for a point on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toward {
    /// The rows after it: a folder line, a card's head.
    Next,
    /// The rows before it: a card's foot and the gap under it.
    Prev,
}

/// A stretch of the list: where it lies, in logical pixels from the list's top, the block it is
/// part of, and the screen rows it draws (empty when it draws none).
#[derive(Debug, Clone, PartialEq)]
pub struct ListSpan {
    pub top: f32,
    pub height: f32,
    pub block: Option<BlockId>,
    pub rows: Range<u32>,
    pub toward: Toward,
}

/// The list as last laid out: its stretches top to bottom, and how far it is scrolled.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ListMap {
    pub offset: f32,
    pub spans: Vec<ListSpan>,
}

/// Where on a screen row a point lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Landing {
    /// On the row, at the point's own column.
    On(u32),
    /// Before the row's first column.
    Start(u32),
    /// After the row's last column.
    End(u32),
}

/// What a point over the list is over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListHit {
    pub landing: Landing,
    pub block: Option<BlockId>,
}

/// Where `y` (logical pixels from the list's visible top) lands, with rows `row` pixels high.
/// `None` for an empty list or one that draws no rows.
pub fn list_hit(map: &ListMap, y: f32, row: f32) -> Option<ListHit> {
    let at = y + map.offset;
    let spans = &map.spans;
    // The stretch at the point, or the last one above it (the gap under a card is its foot's).
    let index = spans.partition_point(|s| s.top <= at).saturating_sub(1);
    let span = spans.get(index)?;
    let block = span.block;
    if !span.rows.is_empty() {
        let within = ((at - span.top).max(0.0) / row.max(1.0)) as u32;
        let last = span.rows.end - 1;
        let landing = match at < span.top {
            true => Landing::Start(span.rows.start),
            false => Landing::On((span.rows.start + within).min(last)),
        };
        return Some(ListHit { landing, block });
    }
    let after = || {
        spans[index..]
            .iter()
            .find(|s| !s.rows.is_empty())
            .map(|s| Landing::Start(s.rows.start))
    };
    let before = || {
        spans[..index]
            .iter()
            .rev()
            .find(|s| !s.rows.is_empty())
            .map(|s| Landing::End(s.rows.end - 1))
    };
    let landing = match span.toward {
        Toward::Next => after().or_else(before),
        Toward::Prev => before().or_else(after),
    }?;
    Some(ListHit { landing, block })
}

#[cfg(test)]
#[path = "list_hit_tests.rs"]
mod tests;
