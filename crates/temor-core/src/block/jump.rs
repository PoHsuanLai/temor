//! Moving between prompts (PLAN §6: ⌘↑ and ⌘↓). Pure: the caller lists each block's prompt row
//! in order and says where the viewport starts; the answer is the block to bring into view.

use crate::vocab::ids::{BlockId, GridRow};

/// The last prompt that starts above `top`, if any. `prompts` is oldest first.
pub fn previous_prompt(prompts: &[(BlockId, GridRow)], top: GridRow) -> Option<BlockId> {
    prompts
        .iter()
        .rev()
        .find(|(_, row)| *row < top)
        .map(|(id, _)| *id)
}

/// The first prompt that starts below `top`, if any. `prompts` is oldest first.
pub fn next_prompt(prompts: &[(BlockId, GridRow)], top: GridRow) -> Option<BlockId> {
    prompts
        .iter()
        .find(|(_, row)| *row > top)
        .map(|(id, _)| *id)
}
