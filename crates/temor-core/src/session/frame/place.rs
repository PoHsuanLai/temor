//! The row arithmetic of the viewport: which rows a block holds, when it heads the pane, where
//! the block tools go and which rows a command line takes. Plain functions on rows; nothing reads
//! a terminal.

/// How many rows of a block must show for its command to head the pane. Fewer, and the header
/// would cover most of what is left of the block.
pub const HEADER_MIN_ROWS: u32 = 2;

/// A block's rows in the screen (0 is the oldest kept row), prompt through last output row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub first: u32,
    pub last: u32,
}

/// The viewport, in screen rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewRows {
    /// The first row shown.
    pub top: u32,
    /// The last row shown.
    pub bottom: u32,
    /// The cursor's row, when it is shown.
    pub cursor: Option<u32>,
}

/// Whether a block heads the viewport: its prompt is above the top and at least
/// [`HEADER_MIN_ROWS`] of its rows show.
pub fn heads_viewport(span: Span, view: &ViewRows) -> bool {
    let shown = span.last.min(view.bottom).saturating_add(1);
    span.first < view.top && shown.saturating_sub(view.top) >= HEADER_MIN_ROWS
}

/// The screen row the tools go on: the block's header row (its prompt's row) when that is in
/// view, else the top row, which the sticky header covers, when the block heads the viewport.
/// `None` when the block has neither, or when the tools, `reach` rows tall, would cover the
/// cursor's row.
pub fn tools_row(span: Span, view: &ViewRows, reach: u32) -> Option<u32> {
    let row = match span.first >= view.top {
        true => span.first,
        false if heads_viewport(span, view) => view.top,
        false => return None,
    };
    if row > span.last.min(view.bottom) {
        return None;
    }
    let covers_cursor = view
        .cursor
        .is_some_and(|cursor| cursor >= row && cursor < row.saturating_add(reach));
    match covers_cursor {
        true => None,
        false => Some(row),
    }
}

/// The rows of a command line: from the row of the input mark to the row before the output
/// mark when that mark is at a line's start, else to the output mark's row. `output` is the
/// mark's row and column; without it the line runs to `fallback_end`, never above `input`.
pub fn command_rows(input: i64, output: Option<(i64, u16)>, fallback_end: i64) -> (i64, i64) {
    let last = match output {
        Some((row, 0)) if row > input => row - 1,
        Some((row, _)) => row,
        None => fallback_end,
    };
    (input, last.max(input))
}

#[cfg(test)]
#[path = "place_tests.rs"]
mod tests;
