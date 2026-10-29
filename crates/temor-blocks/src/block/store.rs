//! The pane's command blocks, oldest first and bounded (PLAN §7). Finished blocks go first when
//! the store is full. The caller drops a block whose prompt the scrollback no longer holds.

use std::collections::VecDeque;

use super::model::{Block, Outcome};
use super::step::BlockOut;
use temor_core::vocab::ids::BlockId;

/// Most blocks kept per pane.
pub const MAX_BLOCKS: usize = 500;

/// The blocks of one pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockStore {
    blocks: VecDeque<Block>,
    cap: usize,
}

impl Default for BlockStore {
    fn default() -> Self {
        Self::with_cap(MAX_BLOCKS)
    }
}

impl BlockStore {
    /// A store that keeps at most `cap` blocks (at least one).
    pub fn with_cap(cap: usize) -> Self {
        Self {
            blocks: VecDeque::new(),
            cap: cap.max(1),
        }
    }

    /// Records what the block machine said. Returns the blocks this removed to make room.
    pub fn apply(&mut self, out: &BlockOut) -> Vec<BlockId> {
        match out {
            BlockOut::CommandStarted(block) | BlockOut::BlockDone(block) => {
                self.upsert(block.clone())
            }
            BlockOut::PromptShown | BlockOut::ShellGone => Vec::new(),
        }
    }

    fn upsert(&mut self, block: Block) -> Vec<BlockId> {
        match self.blocks.iter_mut().find(|b| b.id == block.id) {
            Some(slot) => *slot = block,
            None => self.blocks.push_back(block),
        }
        let mut evicted = Vec::new();
        while self.blocks.len() > self.cap {
            // The oldest finished block goes first; a running one only when nothing has finished.
            let finished = self
                .blocks
                .iter()
                .position(|b| b.outcome() != Outcome::Running);
            let old = match finished {
                Some(at) => self.blocks.remove(at),
                None => self.blocks.pop_front(),
            };
            if let Some(old) = old {
                evicted.push(old.id);
            }
        }
        evicted
    }

    /// Drops every block `live` says is gone (its prompt left the scrollback). Returns them.
    pub fn retain_live(&mut self, live: impl Fn(BlockId) -> bool) -> Vec<BlockId> {
        let mut gone = Vec::new();
        self.blocks.retain(|b| {
            let keep = live(b.id);
            if !keep {
                gone.push(b.id);
            }
            keep
        });
        gone
    }

    /// The block with this id.
    pub fn get(&self, id: BlockId) -> Option<&Block> {
        self.blocks.iter().find(|b| b.id == id)
    }

    /// Every block, oldest first.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &Block> {
        self.blocks.iter()
    }

    /// The newest block.
    pub fn newest(&self) -> Option<&Block> {
        self.blocks.back()
    }

    /// How many blocks are kept.
    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    /// Whether there are no blocks.
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }
}
