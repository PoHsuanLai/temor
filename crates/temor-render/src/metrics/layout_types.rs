//! The pieces of a [`RowLayout`](super::RowLayout): what a caller asks for (rows left out, which
//! end stays put) and what it gets back (slices of the drawn grid).

use temor_vt::{Px, Row};

use super::model::GridFit;

/// Grid rows `first..=last` left out of the picture: the rows below them move up by their height,
/// and nothing is shown where they were.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fold {
    pub first: Row,
    pub last: Row,
}

/// Which end of the pane the rows hold on to when rows are left out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Pin {
    /// The last row stays where it is: the rows above a fold move down.
    #[default]
    Bottom,
    /// The first rows stay where they are: the rows below a fold move up.
    Top,
}

/// Which of the terminal's screens the grid shows. A full-screen program's screen has no rows
/// left out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Screen {
    #[default]
    Primary,
    Alternate,
}

/// A band of the drawn grid shown in the pane: drawn device rows `src_y..src_y + height`, shown
/// from `dst_y` down. Always inside the pane and the picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Slice {
    pub src_y: Px,
    pub height: Px,
    pub dst_y: Px,
}

/// What a [`RowLayout`] is made from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutParams<'a> {
    /// The grid as it is drawn: fitted, and lowered over the blank rows under the screen.
    pub fit: GridFit,
    /// The rows to leave out. Not applied to the alternate screen.
    pub folded: &'a [Fold],
    pub pin: Pin,
    pub screen: Screen,
}
