//! Keeping the chips off the typing line: a long command typed on the chips' own row runs under
//! them, so the ones it reaches are left out (the last, least important, first).

/// Where the chips sit on their row, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChipRow {
    /// The pane's width.
    pub pane_width: f32,
    /// The pane's padding: the chips end this far from the right edge.
    pub inset: f32,
    /// One terminal cell's width. A chip is its characters plus two cells of padding, and the
    /// gap between chips is one cell: an estimate that errs towards hiding.
    pub cell_width: f32,
}

/// How many of the chips, from the first, can be shown. `chars` is each chip's text length.
/// `cursor_right` is where the cursor ends when it is on the chips' row, `None` when it is on
/// another row (then all chips fit). The chips keep one cell of room from the cursor.
pub fn chips_that_fit(chars: &[usize], row: ChipRow, cursor_right: Option<f32>) -> usize {
    let Some(cursor_right) = cursor_right else {
        return chars.len();
    };
    let left_of = |kept: usize| {
        let widths: f32 = chars[..kept]
            .iter()
            .map(|&n| (n as f32 + 2.0) * row.cell_width)
            .sum();
        let gaps = kept.saturating_sub(1) as f32 * row.cell_width;
        row.pane_width - row.inset - widths - gaps
    };
    (0..=chars.len())
        .rev()
        .find(|&kept| kept == 0 || left_of(kept) >= cursor_right + row.cell_width)
        .unwrap_or(0)
}
