//! The pane's block log (PLAN §9 blocks): the mark log of `temor-core` (the block machine, its
//! store, and which anchors locate each block's marks), seen through the terminal's anchor ids,
//! and the chips each block's header carried. The rules are in `temor-core`; the session reads
//! the terminal into the log's inputs and carries out what it says.

use temor_core::block::{BlockState, BlockStore};
use temor_core::session::{Anchor, MarkLog};
use temor_core::vocab::ids::BlockId;
use temor_vt::AnchorId;

/// The anchors of one block: where its prompt, its typed command, its output and its end are.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct BlockAnchors {
    pub(super) prompt: Option<AnchorId>,
    pub(super) input: Option<AnchorId>,
    pub(super) output: Option<AnchorId>,
    pub(super) end: Option<AnchorId>,
}

/// The core's handle for a terminal anchor.
pub(super) fn handle(id: AnchorId) -> Anchor {
    Anchor(id.0)
}

/// The terminal's anchor for a core handle.
pub(super) fn anchor_id(anchor: Anchor) -> AnchorId {
    AnchorId(anchor.0)
}

/// Everything the session knows about the pane's blocks.
#[derive(Debug, Default)]
pub(super) struct BlockLog {
    pub(super) marks: MarkLog,
    /// What each finished block's card says of its output.
    pub(super) notes: std::collections::BTreeMap<BlockId, temor_core::block::Note>,
}

impl BlockLog {
    /// Where the block machine stands.
    pub(super) fn machine(&self) -> &BlockState {
        self.marks.machine()
    }

    /// The blocks, oldest first.
    pub(super) fn store(&self) -> &BlockStore {
        self.marks.store()
    }

    /// The anchors of a block.
    pub(super) fn anchors_of(&self, id: BlockId) -> Option<BlockAnchors> {
        let held = self.marks.anchors_of(id)?;
        Some(BlockAnchors {
            prompt: held.prompt.map(anchor_id),
            input: held.input.map(anchor_id),
            output: held.output.map(anchor_id),
            end: held.end.map(anchor_id),
        })
    }

    /// The anchor of the prompt being typed at.
    pub(super) fn prompt(&self) -> Option<AnchorId> {
        self.marks.prompt().map(anchor_id)
    }

    /// The anchor of where the typed command starts.
    pub(super) fn input(&self) -> Option<AnchorId> {
        self.marks.input().map(anchor_id)
    }
}
