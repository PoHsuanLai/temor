//! The pane as a list of blocks (PLAN: block list): each finished command a card with its own
//! drawing of its output, the running one live, and the composer last. A full-screen program's
//! screen is drawn whole instead.

pub mod composer;
mod more;
pub mod pane;
pub mod segments;
pub mod tiles;

pub use composer::ComposerActions;
pub use pane::{BlockPane, CardActions, CardOffers, ComposerView};
