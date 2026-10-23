//! What a person has done to a pane's blocks: which are picked, which have their output folded
//! away and which are bookmarked. One state and one step, so every way of changing them (a
//! click, an arrow, a key, a menu) goes through the same rules. The rules for picks are
//! [`Picks`]; this puts them with the folds and bookmarks.

use std::collections::BTreeSet;

use crate::block::{PickClick, PickStep, Picks, next_bookmark};
use crate::vocab::ids::BlockId;

/// The picked, folded and bookmarked blocks of one pane.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Annotations {
    picks: Picks,
    folds: BTreeSet<BlockId>,
    bookmarks: BTreeSet<BlockId>,
}

impl Annotations {
    /// Whether nothing is picked.
    pub fn nothing_picked(&self) -> bool {
        self.picks.is_empty()
    }

    /// Whether `id` is picked.
    pub fn is_picked(&self, id: BlockId) -> bool {
        self.picks.contains(id)
    }

    /// The block the arrows move, if any.
    pub fn active_pick(&self) -> Option<BlockId> {
        self.picks.active()
    }

    /// The picked blocks in `order`'s order; blocks no longer in it are left out.
    pub fn picked_in(&self, order: &[BlockId]) -> Vec<BlockId> {
        self.picks.in_order(order)
    }

    /// Whether `id`'s output is folded away.
    pub fn is_folded(&self, id: BlockId) -> bool {
        self.folds.contains(&id)
    }

    /// The blocks whose output is folded away.
    pub fn folded(&self) -> impl Iterator<Item = BlockId> + '_ {
        self.folds.iter().copied()
    }

    /// Whether `id` is bookmarked.
    pub fn is_bookmarked(&self, id: BlockId) -> bool {
        self.bookmarks.contains(&id)
    }

    /// Whether some block is bookmarked.
    pub fn any_bookmarked(&self) -> bool {
        !self.bookmarks.is_empty()
    }

    /// Whether every picked block is bookmarked (and some block is picked).
    pub fn picks_bookmarked(&self, order: &[BlockId]) -> bool {
        let picked = self.picked_in(order);
        !picked.is_empty() && picked.iter().all(|id| self.bookmarks.contains(id))
    }

    /// The picked blocks that can fold, among `foldable`.
    fn foldable_picks(&self, order: &[BlockId], foldable: &BTreeSet<BlockId>) -> Vec<BlockId> {
        self.picked_in(order)
            .into_iter()
            .filter(|id| foldable.contains(id))
            .collect()
    }

    /// Whether every picked block that can fold is folded (and some can).
    pub fn picks_folded(&self, order: &[BlockId], foldable: &BTreeSet<BlockId>) -> bool {
        let picks = self.foldable_picks(order, foldable);
        !picks.is_empty() && picks.iter().all(|id| self.folds.contains(id))
    }
}

/// What changes the annotations. `order` is the blocks that can be picked, oldest first.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AnnotateIn {
    /// A click on a block.
    Click {
        id: BlockId,
        how: PickClick,
        order: Vec<BlockId>,
    },
    /// An arrow: the pick moves.
    Step { step: PickStep, order: Vec<BlockId> },
    /// Shift and an arrow: the run from the anchor grows or shrinks.
    Extend { step: PickStep, order: Vec<BlockId> },
    /// Every block.
    SelectAll { order: Vec<BlockId> },
    /// Nothing picked.
    ClearPicks,
    /// The pick moves to the nearest bookmark that way.
    StepBookmark { step: PickStep, order: Vec<BlockId> },
    /// Bookmarks the picked blocks, or takes their bookmarks away when every one has one.
    ToggleBookmarks { order: Vec<BlockId> },
    /// Folds the picked blocks' output (those in `foldable`), or brings it back when every one
    /// of them is folded.
    ToggleFolds {
        order: Vec<BlockId>,
        foldable: BTreeSet<BlockId>,
    },
    /// Folds one block's output or brings it back; `foldable` is whether it has output to fold.
    ToggleFold { id: BlockId, foldable: bool },
}

/// What the edge does after a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AnnotateOut {
    /// Bring the active pick into view.
    Reveal,
    /// The folded blocks changed, so the rows must be laid out again.
    FoldsChanged,
}

/// The annotations after `input`, and what to do about it.
pub fn annotate_step(mut state: Annotations, input: AnnotateIn) -> (Annotations, Vec<AnnotateOut>) {
    let mut outs = Vec::new();
    match input {
        AnnotateIn::Click { id, how, order } => state.picks.click(id, how, &order),
        AnnotateIn::Step { step, order } => {
            state.picks.step(step, &order);
            outs.push(AnnotateOut::Reveal);
        }
        AnnotateIn::Extend { step, order } => {
            state.picks.extend(step, &order);
            outs.push(AnnotateOut::Reveal);
        }
        AnnotateIn::SelectAll { order } => state.picks.select_all(&order),
        AnnotateIn::ClearPicks => state.picks.clear(),
        AnnotateIn::StepBookmark { step, order } => {
            if let Some(id) = next_bookmark(&order, &state.bookmarks, state.picks.active(), step) {
                state.picks.click(id, PickClick::Only, &order);
                outs.push(AnnotateOut::Reveal);
            }
        }
        AnnotateIn::ToggleBookmarks { order } => {
            let picked = state.picked_in(&order);
            if picked.iter().all(|id| state.bookmarks.contains(id)) {
                for id in &picked {
                    state.bookmarks.remove(id);
                }
            } else {
                state.bookmarks.extend(picked);
            }
        }
        AnnotateIn::ToggleFolds { order, foldable } => {
            let picks = state.foldable_picks(&order, &foldable);
            if state.picks_folded(&order, &foldable) {
                for id in &picks {
                    state.folds.remove(id);
                }
            } else {
                state.folds.extend(picks);
            }
            outs.push(AnnotateOut::FoldsChanged);
        }
        AnnotateIn::ToggleFold { id, foldable } => {
            if state.folds.remove(&id) || (foldable && state.folds.insert(id)) {
                outs.push(AnnotateOut::FoldsChanged);
            }
        }
    }
    (state, outs)
}
