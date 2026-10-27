//! What temor keeps between launches (PLAN §7, §2b P24b): the layout in the store ([`Kept`]),
//! each pane's files beside it ([`PaneFiles`]), when a change is written ([`SavePace`]), and the
//! files an older temor wrote, read in once ([`older`]). The window hands over what it has and
//! carries out what a restore says to start.

pub mod checkpoint;
mod kept;
pub mod older;
mod pace;
mod panes;
pub mod private;
pub mod replay;
pub mod sidecar;

pub use kept::{Kept, Laying, Starting, Through};
pub use pace::{Pace, SavePace};
pub use panes::{KeptPane, PaneFiles};
