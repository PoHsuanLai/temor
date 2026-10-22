//! The pieces of a [`RowLayout`](super::RowLayout): what a caller asks for (room under rows, which
//! end stays put) and what it gets back (slices of the drawn grid, gaps, what a point is over).

use temor_vt::{Px, Row, SurfacePoint};

use super::model::GridFit;

/// Room of `height` device pixels under grid row `after`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Insert {
    pub after: Row,
    pub height: Px,
}

/// Grid rows `first..=last` left out of the picture: the rows below them move up by their height,
/// and nothing is shown where they were.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fold {
    pub first: Row,
    pub last: Row,
}

/// Which end of the pane the rows hold on to when room is made between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Pin {
    /// The last row stays where it is: the rows above the room move up, and those pushed above
    /// the pane are cut.
    #[default]
    Bottom,
    /// The first rows stay reachable: the rows move up only into the blank rows the grid was
    /// lowered over, and the rest of the room pushes the rows below it down.
    Top,
}

/// Which of the terminal's screens the grid shows. A full-screen program's screen has no room
/// made in it.
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

/// The room under row `after` as it is shown: device rows `dst_y..dst_y + height` of the pane,
/// cut to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Gap {
    pub after: Row,
    pub dst_y: Px,
    pub height: Px,
}

/// What a pane point is over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum GridHit {
    /// The drawn grid, at this point of the picture (map it to a cell with
    /// [`GridFit::cell_at`]).
    Grid(SurfacePoint),
    /// The room under row `after`.
    Gap { after: Row },
}

/// What a [`RowLayout`] is made from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutParams<'a> {
    /// The grid as it is drawn: fitted, and lowered by `lower`.
    pub fit: GridFit,
    /// How many blank rows the grid was lowered over.
    pub lower: u16,
    /// The room to make; inserts under rows the grid does not have are left out.
    pub inserts: &'a [Insert],
    /// The rows to leave out. Room under a folded row is left out with it. Not applied to the
    /// alternate screen.
    pub folded: &'a [Fold],
    pub pin: Pin,
    pub screen: Screen,
}
