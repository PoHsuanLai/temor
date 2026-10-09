//! Moving panes between trees: graft another tree in beside a pane, and tear a pane off into a
//! tree of its own. Both return new trees, as the other operations do.

use super::model::{Direction, Handle, MOST_PANES, Node, Share, SplitTree};
use crate::error::CoreError;
use crate::vocab::ids::PaneId;

/// The handle `index` as a `Handle`; a tree holds at most `2 * MOST_PANES` nodes, which fits.
fn handle(index: usize) -> Handle {
    Handle(u16::try_from(index).unwrap_or(u16::MAX))
}

/// `nodes` moved so that its root sits at `base` in the enclosing array.
fn rebased(nodes: &[Node], base: usize) -> impl Iterator<Item = Node> + '_ {
    nodes.iter().map(move |node| match *node {
        Node::Leaf(pane) => Node::Leaf(pane),
        Node::Split {
            axis,
            ratio,
            first,
            second,
        } => Node::Split {
            axis,
            ratio,
            first: handle(base + first.index()),
            second: handle(base + second.index()),
        },
    })
}

impl SplitTree {
    /// The tree with `other` put beside pane `at`, on the side `towards` points at, each half
    /// of the space. Refused when `other` shares a pane with this tree (grafting a tree onto
    /// itself is one such case) or when the result would hold more than [`MOST_PANES`].
    pub fn graft(
        &self,
        at: PaneId,
        other: &SplitTree,
        towards: Direction,
    ) -> Result<SplitTree, CoreError> {
        if !self.contains(at) {
            return Err(CoreError::UnknownPane(at.0));
        }
        if let Some(clash) = other.panes().into_iter().find(|pane| self.contains(*pane)) {
            return Err(CoreError::DuplicatePane(clash.0));
        }
        if self.panes().len() + other.panes().len() > MOST_PANES {
            return Err(CoreError::TreeFull);
        }
        let other_first = towards.is_towards_first();
        let mut nodes = Vec::with_capacity(self.nodes.len() + other.nodes.len() + 1);
        self.copy_with(Handle::ROOT, &mut nodes, &mut |pane, out| {
            if pane != at {
                out.push(Node::Leaf(pane));
                return;
            }
            let here = out.len();
            // The grafted tree takes the first side when `towards` points at it (left or
            // top); the pane `at` keeps the other side.
            let (first, second) = if other_first {
                (here + 1, here + 1 + other.nodes.len())
            } else {
                (here + 1, here + 2)
            };
            out.push(Node::Split {
                axis: towards.axis(),
                ratio: Share::HALF,
                first: handle(first),
                second: handle(second),
            });
            if other_first {
                out.extend(rebased(&other.nodes, here + 1));
                out.push(Node::Leaf(at));
            } else {
                out.push(Node::Leaf(at));
                out.extend(rebased(&other.nodes, here + 2));
            }
        });
        Ok(SplitTree {
            nodes,
            zoomed: None,
        })
    }

    /// Takes `pane` out of this tab: the rest of the tree (its sibling takes its place) and the
    /// pane alone, as a tree of its own. Refused for a tab's only pane, which closes the tab
    /// instead.
    pub fn tear(&self, pane: PaneId) -> Result<(SplitTree, SplitTree), CoreError> {
        if !self.contains(pane) {
            return Err(CoreError::UnknownPane(pane.0));
        }
        match self.remove(pane)? {
            super::ops::Closed::Remaining(rest) => Ok((rest, SplitTree::single(pane))),
            super::ops::Closed::Last => Err(CoreError::LastPane),
        }
    }
}
