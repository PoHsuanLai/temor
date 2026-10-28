//! The operations that change a tree: split, remove, equalize, resize, zoom. Each returns a new
//! tree in canonical pre-order, so two trees with the same shape are equal however they were
//! made.

use super::model::{Axis, Direction, Handle, MOST_PANES, Node, Share, SplitTree, WindowDelta};
use temor_core::error::CoreError;
use temor_core::vocab::ids::PaneId;

/// What closing a pane leaves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Closed {
    /// The other panes, re-laid out without it.
    Remaining(SplitTree),
    /// It was the tab's only pane: the tab closes.
    Last,
}

/// The handle of the node at `index`; node arrays never pass `2 * MOST_PANES`, which fits.
/// One end of a subtree's panes in reading order.
#[derive(Clone, Copy)]
enum End {
    First,
    Last,
}

fn handle_at(index: usize) -> Handle {
    Handle(u16::try_from(index).unwrap_or(u16::MAX)) // len <= 2 * MOST_PANES < u16::MAX
}

impl SplitTree {
    /// The tree with `at` split in two: `at` keeps its side and `new` takes the side `towards`
    /// points at, each with half. Splitting unzooms, as in Ghostty.
    pub fn split(
        &self,
        at: PaneId,
        new: PaneId,
        towards: Direction,
    ) -> Result<SplitTree, CoreError> {
        if !self.contains(at) {
            return Err(CoreError::UnknownPane(at.0));
        }
        if self.contains(new) {
            return Err(CoreError::DuplicatePane(new.0));
        }
        if self.panes().len() >= MOST_PANES {
            return Err(CoreError::TreeFull);
        }
        let (first, second) = if towards.is_towards_first() {
            (new, at)
        } else {
            (at, new)
        };
        let mut nodes = Vec::with_capacity(self.nodes.len() + 2);
        self.copy_with(Handle::ROOT, &mut nodes, &mut |pane, out| {
            if pane != at {
                out.push(Node::Leaf(pane));
                return;
            }
            let here = out.len();
            out.push(Node::Split {
                axis: towards.axis(),
                ratio: Share::HALF,
                first: handle_at(here + 1),
                second: handle_at(here + 2),
            });
            out.push(Node::Leaf(first));
            out.push(Node::Leaf(second));
        });
        Ok(SplitTree {
            nodes,
            zoomed: None,
        })
    }

    /// The tree without `pane`: its sibling takes its parent's place. A zoom on another pane
    /// survives; a zoom on `pane` goes with it.
    pub fn remove(&self, pane: PaneId) -> Result<Closed, CoreError> {
        let gone = self.handle_of(pane).ok_or(CoreError::UnknownPane(pane.0))?;
        if gone == Handle::ROOT {
            return Ok(Closed::Last);
        }
        let mut nodes = Vec::with_capacity(self.nodes.len());
        self.copy_without(Handle::ROOT, gone, &mut nodes);
        Ok(Closed::Remaining(SplitTree {
            nodes,
            zoomed: self.zoomed.filter(|zoomed| *zoomed != pane),
        }))
    }

    /// Every split's share set from its children's weights along its axis, so panes in a row
    /// (or a column) come out the same size. Idempotent.
    pub fn equalize(&self) -> SplitTree {
        let nodes = self
            .nodes
            .iter()
            .map(|node| match *node {
                Node::Leaf(pane) => Node::Leaf(pane),
                Node::Split {
                    axis,
                    first,
                    second,
                    ..
                } => {
                    let (a, b) = (self.weight(first, axis), self.weight(second, axis));
                    let even = a * i64::from(Share::WHOLE) / (a + b).max(1);
                    // At most MOST_PANES panes keeps every even share inside Share's range.
                    debug_assert_eq!(i64::from(Share::new(even).millionths()), even);
                    Node::Split {
                        axis,
                        ratio: Share::new(even),
                        first,
                        second,
                    }
                }
            })
            .collect();
        SplitTree {
            nodes,
            zoomed: self.zoomed,
        }
    }

    /// The divider of the nearest split along `axis` that holds `from`, moved by `delta` of the
    /// whole window. The delta becomes a change relative to that split's own size, and the
    /// share is clamped. With no such split, the tree is unchanged.
    pub fn resize(
        &self,
        from: PaneId,
        axis: Axis,
        delta: WindowDelta,
    ) -> Result<SplitTree, CoreError> {
        let leaf = self.handle_of(from).ok_or(CoreError::UnknownPane(from.0))?;
        let Some(split) = self.nearest_split(leaf, axis) else {
            return Ok(self.clone());
        };
        let slots = self.spatial();
        let (whole, extent) = match axis {
            Axis::Row => (slots[0].width, slots[split.index()].width),
            Axis::Column => (slots[0].height, slots[split.index()].height),
        };
        if extent == 0 {
            return Ok(self.clone());
        }
        let mut nodes = self.nodes.clone();
        if let Some(Node::Split { ratio, .. }) = nodes.get_mut(split.index()) {
            let moved = i128::from(delta.0) * i128::from(whole) / i128::from(extent);
            let wanted = i128::from(ratio.millionths()) + moved;
            *ratio = Share::new(i64::try_from(wanted).unwrap_or(i64::MAX));
        }
        Ok(SplitTree {
            nodes,
            zoomed: self.zoomed,
        })
    }

    /// The split whose divider lies between `before` and `after` (the last pane of its first
    /// child and the first pane of its second) given `share`. Two panes next to each other in
    /// reading order have exactly one divider between them; panes that are not (a view of the
    /// tab that is already stale) leave the tree unchanged.
    pub fn set_share(&self, before: PaneId, after: PaneId, share: Share) -> SplitTree {
        let mut nodes = self.nodes.clone();
        let between = |first: Handle, second: Handle| {
            self.end_pane(first, End::Last) == Some(before)
                && self.end_pane(second, End::First) == Some(after)
        };
        if let Some(Node::Split { ratio, .. }) = nodes.iter_mut().find(|node| match node {
            Node::Split { first, second, .. } => between(*first, *second),
            Node::Leaf(_) => false,
        }) {
            *ratio = share;
        }
        SplitTree {
            nodes,
            zoomed: self.zoomed,
        }
    }

    /// The first or last pane under `at`.
    fn end_pane(&self, at: Handle, end: End) -> Option<PaneId> {
        match self.node(at)? {
            Node::Leaf(pane) => Some(pane),
            Node::Split { first, second, .. } => match end {
                End::First => self.end_pane(first, end),
                End::Last => self.end_pane(second, end),
            },
        }
    }

    /// The tree with `pane` shown over the whole tab. A tab that is not split has nothing to zoom.
    pub fn zoom(&self, pane: PaneId) -> Result<SplitTree, CoreError> {
        if !self.contains(pane) {
            return Err(CoreError::UnknownPane(pane.0));
        }
        Ok(SplitTree {
            nodes: self.nodes.clone(),
            zoomed: self.is_split().then_some(pane),
        })
    }

    /// The tree with every pane shown.
    pub fn unzoom(&self) -> SplitTree {
        SplitTree {
            nodes: self.nodes.clone(),
            zoomed: None,
        }
    }

    /// The node each node hangs from; the root has none.
    pub(super) fn parents(&self) -> Vec<Option<Handle>> {
        let mut parents = vec![None; self.nodes.len()];
        for (index, node) in self.nodes.iter().enumerate() {
            if let Node::Split { first, second, .. } = node {
                for child in [first, second] {
                    if let Some(slot) = parents.get_mut(child.index()) {
                        *slot = Some(handle_at(index));
                    }
                }
            }
        }
        parents
    }

    /// The deepest split along `axis` above `leaf`.
    fn nearest_split(&self, leaf: Handle, axis: Axis) -> Option<Handle> {
        let parents = self.parents();
        let mut at = parents.get(leaf.index()).copied().flatten();
        while let Some(handle) = at {
            if matches!(self.node(handle), Some(Node::Split { axis: a, .. }) if a == axis) {
                return Some(handle);
            }
            at = parents.get(handle.index()).copied().flatten();
        }
        None
    }

    /// How many panes sit side by side along `axis` under `at`: a split along another axis
    /// counts as one, as in Ghostty.
    fn weight(&self, at: Handle, axis: Axis) -> i64 {
        match self.node(at) {
            Some(Node::Split {
                axis: own,
                first,
                second,
                ..
            }) if own == axis => self.weight(first, axis) + self.weight(second, axis),
            _ => 1,
        }
    }

    /// Copies the subtree at `at` into `out` in pre-order, letting `on_leaf` write each leaf.
    pub(super) fn copy_with(
        &self,
        at: Handle,
        out: &mut Vec<Node>,
        on_leaf: &mut dyn FnMut(PaneId, &mut Vec<Node>),
    ) -> Handle {
        let here = handle_at(out.len());
        match self.node(at) {
            Some(Node::Leaf(pane)) => on_leaf(pane, out),
            Some(Node::Split {
                axis,
                ratio,
                first,
                second,
            }) => {
                out.push(Node::Leaf(PaneId(0)));
                let first = self.copy_with(first, out, on_leaf);
                let second = self.copy_with(second, out, on_leaf);
                out[here.index()] = Node::Split {
                    axis,
                    ratio,
                    first,
                    second,
                };
            }
            None => {}
        }
        here
    }

    /// Copies the subtree at `at` into `out` in pre-order, leaving out the node `gone` and the
    /// split that held it.
    fn copy_without(&self, at: Handle, gone: Handle, out: &mut Vec<Node>) -> Handle {
        match self.node(at) {
            Some(Node::Split { first, second, .. }) if first == gone => {
                self.copy_without(second, gone, out)
            }
            Some(Node::Split { first, second, .. }) if second == gone => {
                self.copy_without(first, gone, out)
            }
            Some(Node::Split {
                axis,
                ratio,
                first,
                second,
            }) => {
                let here = handle_at(out.len());
                out.push(Node::Leaf(PaneId(0)));
                let first = self.copy_without(first, gone, out);
                let second = self.copy_without(second, gone, out);
                out[here.index()] = Node::Split {
                    axis,
                    ratio,
                    first,
                    second,
                };
                here
            }
            Some(Node::Leaf(pane)) => {
                out.push(Node::Leaf(pane));
                handle_at(out.len() - 1)
            }
            None => handle_at(out.len()),
        }
    }
}
