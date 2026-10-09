//! Command blocks from OSC 133 (D5): the shell's marks as a machine, the blocks it makes, and the
//! pure rules for moving between them.

mod elapsed;
mod jump;
mod model;
mod step;
mod store;
#[cfg(test)]
mod store_tests;
#[cfg(test)]
mod tests;

pub use elapsed::format_elapsed;
pub use jump::{next_prompt, previous_prompt};
pub use model::{Author, Block, Outcome, Taint};
pub use step::{BlockIn, BlockOut, BlockState, block_step};
pub use store::{BlockStore, MAX_BLOCKS};
