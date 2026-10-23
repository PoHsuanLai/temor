//! What the terminal said of the blocks, as plain values: the edge reads it, and a [`Frame`] is
//! made of it.

use crate::block::Outcome;
use crate::vocab::ids::BlockId;

use super::card::Lead;

/// A mark's place in the screen: its row and column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spot {
    pub row: u32,
    pub col: u16,
}

/// Where a block's end mark is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    /// There is none: the command still runs.
    Running,
    At(Spot),
    /// The block has one that the terminal no longer holds.
    Lost,
}

/// One block as the terminal showed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockRead {
    pub id: BlockId,
    /// The command as typed.
    pub command: String,
    pub outcome: Outcome,
    pub took_ms: Option<u64>,
    /// The prompt's mark; `None` once the scrollback took it.
    pub prompt: Option<Spot>,
    /// The output's mark.
    pub output: Option<Spot>,
    /// The typed command's mark.
    pub input: Option<Spot>,
    pub end: End,
    /// What the prompt's first row is.
    pub lead: Lead,
    /// The columns the header line's text takes; read only for a prompt in the viewport.
    pub header_cols: u32,
    pub picked: bool,
    pub bookmarked: bool,
}

/// The prompt being typed at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveRead {
    pub prompt: Spot,
    pub lead: Lead,
    pub input: Option<Spot>,
}

/// Everything the views need from the terminal to place the blocks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Read {
    /// The viewport's first screen row, and how many rows it has.
    pub top: u32,
    pub len: u32,
    /// The cursor's screen row, when it is shown.
    pub cursor: Option<u32>,
    /// The first viewport row shown whole, below the rows the room under a block pushed out.
    pub first_shown: u32,
    /// The store's blocks, oldest first.
    pub blocks: Vec<BlockRead>,
    pub live: Option<LiveRead>,
}
