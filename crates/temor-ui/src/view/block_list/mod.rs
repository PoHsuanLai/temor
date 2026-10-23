//! The pane as a list of blocks (PLAN: block list): each finished command a card with its own
//! drawing of its output, the running one live, and the composer last. Behind
//! `TEMOR_BLOCK_LIST=1` until it replaces the cards laid over the grid.

pub mod pane;
pub mod segments;

pub use crate::session::block_list_on;
pub use pane::BlockPane;
