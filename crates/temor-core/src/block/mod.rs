//! Command blocks from OSC 133 (D5): the shell's marks as a machine, the blocks it makes, and the
//! pure rules for picking them.

mod elapsed;
mod face;
mod model;
mod picks;
#[cfg(test)]
mod picks_tests;
mod step;
mod store;
#[cfg(test)]
mod store_tests;
#[cfg(test)]
mod tests;

pub use elapsed::format_elapsed;
pub use face::{Note, NoteTone, age_strength, bar_share, glyph_bars, summarize};
pub use model::{Author, Block, Outcome, Taint};
pub use picks::{PickClick, PickStep, Picks, next_bookmark};
pub use step::{BlockIn, BlockOut, BlockState, block_step};
pub use store::{BlockStore, MAX_BLOCKS};
