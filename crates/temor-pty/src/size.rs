//! The size a terminal is told: cells and pixels.

pub use temor_schema::units::{Cols, Px, Rows};

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
