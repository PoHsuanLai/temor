//! The row arithmetic of the viewport: which rows a block holds and which rows a command line
//! takes. Plain functions on rows; nothing reads a terminal.

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
