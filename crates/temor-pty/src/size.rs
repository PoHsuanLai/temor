//! The size a terminal is told: cells and pixels.

/// A width in cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Cols(pub u16);

/// A height in cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Rows(pub u16);

/// A length in whole device pixels, as `TIOCSWINSZ` carries it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Px(pub u16);

/// The grid in cells, plus the text area's size in pixels (0 when unknown).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct GridSize {
    pub cols: Cols,
    pub rows: Rows,
    pub width: Px,
    pub height: Px,
}

impl GridSize {
    /// A grid with no pixel size.
    pub fn cells(cols: Cols, rows: Rows) -> Self {
        Self {
            cols,
            rows,
            width: Px(0),
            height: Px(0),
        }
    }
}
