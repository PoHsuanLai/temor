//! Picked blocks, as Warp picks them: a click picks one block, Shift+click picks the run from the
//! last one picked, Ctrl+click adds or drops one, the arrows move the pick and Shift+arrows grow
//! it, and one key picks them all. Copy then takes every picked block whole. Bookmarked blocks are
//! the ones the bookmark keys step between. Pure: `order` is the blocks that can be picked,
//! oldest first.

use std::collections::BTreeSet;

use temor_core::vocab::ids::BlockId;

/// How a click on a block changes the picks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickClick {
    /// This block alone.
    Only,
    /// Every block from the anchor to this one.
    Range,
    /// This block in or out, the others kept.
    Toggle,
}

/// Which way an arrow moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickStep {
    Older,
    Newer,
}

/// The picked blocks, the one a range grows from (`anchor`) and the one the arrows move
/// (`active`). Empty: nothing picked.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Picks {
    picked: BTreeSet<BlockId>,
    anchor: Option<BlockId>,
    active: Option<BlockId>,
}

fn place(order: &[BlockId], id: BlockId) -> Option<usize> {
    order.iter().position(|&b| b == id)
}

impl Picks {
    /// Whether nothing is picked.
    pub fn is_empty(&self) -> bool {
        self.picked.is_empty()
    }

    /// Whether `id` is picked.
    pub fn contains(&self, id: BlockId) -> bool {
        self.picked.contains(&id)
    }

    /// The block the arrows move, if any.
    pub fn active(&self) -> Option<BlockId> {
        self.active
    }

    /// The picked blocks in `order`'s order; blocks no longer in it are left out.
    pub fn in_order(&self, order: &[BlockId]) -> Vec<BlockId> {
        order
            .iter()
            .copied()
            .filter(|id| self.picked.contains(id))
            .collect()
    }

    /// Picks nothing.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    fn only(&mut self, id: BlockId) {
        self.picked = BTreeSet::from([id]);
        self.anchor = Some(id);
        self.active = Some(id);
    }

    /// A click on `id`.
    pub fn click(&mut self, id: BlockId, how: PickClick, order: &[BlockId]) {
        match how {
            PickClick::Only => self.only(id),
            PickClick::Toggle => {
                if self.picked.remove(&id) {
                    self.active = self.picked.iter().next_back().copied();
                    self.anchor = self.active;
                } else {
                    self.picked.insert(id);
                    self.anchor = Some(id);
                    self.active = Some(id);
                }
            }
            PickClick::Range => {
                let from = self.anchor.and_then(|a| place(order, a));
                match (from, place(order, id)) {
                    (Some(from), Some(to)) => {
                        let (lo, hi) = (from.min(to), from.max(to));
                        self.picked = order[lo..=hi].iter().copied().collect();
                        self.active = Some(id);
                    }
                    _ => self.only(id),
                }
            }
        }
    }

    /// An arrow: the pick moves to the block before or after the active one, alone. Past the
    /// newest block nothing is picked (the person is back at the prompt); past the oldest the
    /// pick stays. With nothing picked, `Older` picks the newest block.
    pub fn step(&mut self, step: PickStep, order: &[BlockId]) {
        let at = self.active.and_then(|a| place(order, a));
        let next = match (at, step) {
            (None, PickStep::Older) => order.len().checked_sub(1),
            (None, PickStep::Newer) => None,
            (Some(at), PickStep::Older) => Some(at.saturating_sub(1)),
            (Some(at), PickStep::Newer) => Some(at + 1).filter(|&n| n < order.len()),
        };
        match next {
            Some(n) => self.only(order[n]),
            None => self.clear(),
        }
    }

    /// Every block, the newest the one the arrows move.
    pub fn select_all(&mut self, order: &[BlockId]) {
        self.picked = order.iter().copied().collect();
        self.anchor = order.first().copied();
        self.active = order.last().copied();
    }

    /// Shift and an arrow: the run from the anchor grows (or shrinks) by one block that way.
    pub fn extend(&mut self, step: PickStep, order: &[BlockId]) {
        let (Some(anchor), Some(active)) = (
            self.anchor.and_then(|a| place(order, a)),
            self.active.and_then(|a| place(order, a)),
        ) else {
            return self.step(PickStep::Older, order);
        };
        let to = match step {
            PickStep::Older => active.saturating_sub(1),
            PickStep::Newer => (active + 1).min(order.len() - 1),
        };
        let (lo, hi) = (anchor.min(to), anchor.max(to));
        self.picked = order[lo..=hi].iter().copied().collect();
        self.active = Some(order[to]);
    }
}

/// The bookmarked block nearest the active pick going `step`'s way (from the prompt, with
/// nothing picked: the newest bookmark going `Older`). None when there is none that way.
pub fn next_bookmark(
    order: &[BlockId],
    marked: &BTreeSet<BlockId>,
    from: Option<BlockId>,
    step: PickStep,
) -> Option<BlockId> {
    let at = from.and_then(|id| place(order, id));
    let marked_at = |n: &usize| marked.contains(&order[*n]);
    match (step, at) {
        (PickStep::Older, Some(at)) => (0..at).rev().find(marked_at),
        (PickStep::Older, None) => (0..order.len()).rev().find(marked_at),
        (PickStep::Newer, Some(at)) => (at + 1..order.len()).find(marked_at),
        (PickStep::Newer, None) => None,
    }
    .map(|n| order[n])
}
