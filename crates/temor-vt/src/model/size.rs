//! Grid sizes, pixel sizes, counts and positions.

use ds_core::word::Word;

/// A number of columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Cols(pub u16);

/// A number of rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Rows(pub u16);

/// A zero-based row from the top of the viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Row(pub u16);

/// A zero-based column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Col(pub u16);

/// The terminal's grid in cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GridSize {
    pub cols: Cols,
    pub rows: Rows,
}

impl GridSize {
    /// A grid of `cols` by `rows` cells.
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols: Cols(cols),
            rows: Rows(rows),
        }
    }
}

/// A length in device pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Px(pub u32);

/// The size of one cell in pixels, used for pixel-based reports and image placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CellPx {
    pub width: Px,
    pub height: Px,
}

impl CellPx {
    /// A cell `width` by `height` pixels.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width: Px(width),
            height: Px(height),
        }
    }
}

/// A number of bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ByteCount(pub usize);

/// A number of lines (scrollback length).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LineCount(pub usize);

/// A number of rows in the whole screen (scrollback included).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RowCount(pub u64);

/// A signed number of rows to move by; negative moves towards older output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RowDelta(pub isize);

/// Where to move the viewport over the scrollback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewportScroll {
    /// The oldest line kept.
    Top,
    /// The live screen.
    Bottom,
    /// Move by a number of rows.
    Delta(RowDelta),
    /// Put this screen row (0 is the oldest kept) at the top.
    Row(RowCount),
}

/// The scrollbar the viewport implies: `offset..offset + len` of `total` rows is visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Scrollbar {
    pub total: RowCount,
    pub offset: RowCount,
    pub len: RowCount,
}

/// Which coordinate space a [`GridPoint`]'s row is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum PointSpace {
    /// Rows of the live screen (0 is its top row).
    Active,
    /// Rows of what is shown (0 is the top visible row).
    Viewport,
    /// Rows of everything kept, scrollback first (0 is the oldest).
    Screen,
}

/// A cell position in one of the coordinate spaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GridPoint {
    pub space: PointSpace,
    pub col: Col,
    pub row: u32,
}

impl GridPoint {
    /// Column `col` of row `row` in `space`.
    pub fn new(space: PointSpace, col: u16, row: u32) -> Self {
        Self {
            space,
            col: Col(col),
            row,
        }
    }
}
