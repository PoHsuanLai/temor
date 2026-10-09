//! A selection the host sets on the terminal.

use ds_core::word::Word;

use super::GridPoint;

/// How a selection's two ends cover cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum SelectionShape {
    /// Reading order from one end to the other.
    Linear,
    /// The rectangle with the two ends as corners.
    Block,
}

/// A selection between two cells, both included; the ends may be in either order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextSelection {
    pub start: GridPoint,
    pub end: GridPoint,
    pub shape: SelectionShape,
}
