//! What the bands need to know of a block: how it stands, whether it is picked or bookmarked,
//! and which rows its command takes.

use crate::block::Outcome;

/// Where a block stands, as far as its card goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// The prompt is up and the command has not started.
    AtPrompt,
    /// The command started (or finished) with this outcome.
    Ran(Outcome),
}

/// Whether the block is the one the gutter selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    Selected,
    Unselected,
}

/// Whether the person bookmarked the block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Bookmarked,
    Plain,
}

/// Whether a block's first row is the blank line temor's prompt leaves above its header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lead {
    Blank,
    Header,
}

/// One block: its first row in the viewport (negative above it), what that row is, how it
/// stands, and the rows of its command line (OSC 133 B up to C), when they are known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardBlock {
    pub first: i64,
    pub lead: Lead,
    pub stage: Stage,
    pub pick: Pick,
    pub mark: Mark,
    /// The command's rows, first and last, and the column it starts at in the first.
    pub command: Option<(i64, i64, u16)>,
}
