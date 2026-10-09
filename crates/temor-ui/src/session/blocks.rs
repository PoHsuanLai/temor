//! The pane's block log (PLAN §9 blocks): the block machine, its store, and the anchors that
//! locate each block's marks on the grid. The machine and the rules are in `temor-core`; this
//! holds the glue state. Anchors are released when their block leaves the store.

use std::collections::BTreeMap;

use temor_core::block::{BlockState, BlockStore};
use temor_core::vocab::ids::BlockId;
use temor_vt::{AnchorId, PromptEvent};

/// The anchors of one block: where its prompt, its typed command, its output and its end are.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct BlockAnchors {
    pub(super) prompt: Option<AnchorId>,
    pub(super) input: Option<AnchorId>,
    pub(super) output: Option<AnchorId>,
    pub(super) end: Option<AnchorId>,
}

impl BlockAnchors {
    /// Every anchor the block holds.
    pub(super) fn all(self) -> impl Iterator<Item = AnchorId> {
        [self.prompt, self.input, self.output, self.end]
            .into_iter()
            .flatten()
    }
}

/// Everything the session knows about the pane's blocks.
#[derive(Debug)]
pub(super) struct BlockLog {
    pub(super) machine: BlockState,
    pub(super) store: BlockStore,
    pub(super) anchors: BTreeMap<BlockId, BlockAnchors>,
    /// A mark whose anchor has not arrived yet (the anchor event follows its mark).
    pub(super) pending: Option<PromptEvent>,
    /// The prompt and typed-command anchors of the block being typed, before its id exists.
    pub(super) prompt: Option<AnchorId>,
    pub(super) input: Option<AnchorId>,
    /// The id the next command will get.
    pub(super) next_id: u64,
    /// How many primary prompts the shell has marked.
    pub(super) prompts: usize,
}

impl Default for BlockLog {
    fn default() -> Self {
        Self {
            machine: BlockState::Idle,
            store: BlockStore::default(),
            anchors: BTreeMap::new(),
            pending: None,
            prompt: None,
            input: None,
            next_id: 1,
            prompts: 0,
        }
    }
}

impl BlockLog {
    /// The id for the next command, and the count moved past it.
    pub(super) fn allocate(&mut self) -> BlockId {
        let id = BlockId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Forgets the anchors of blocks the store no longer holds, returning them to release.
    pub(super) fn drop_anchors(&mut self, gone: &[BlockId]) -> Vec<AnchorId> {
        gone.iter()
            .filter_map(|id| self.anchors.remove(id))
            .flat_map(BlockAnchors::all)
            .collect()
    }
}
