//! The tree as it is stored (a session's tabs): the node array and the zoomed pane. Reading
//! checks that the array is one tree over distinct panes and re-lays it in canonical order; a
//! zoom on a pane that is not there is dropped rather than refusing the tab.

use super::model::{Handle, MOST_PANES, Node, SplitTree};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use temor_core::error::CoreError;
use temor_core::vocab::ids::PaneId;

#[derive(Serialize, Deserialize)]
struct TreeWire {
    nodes: Vec<Node>,
    #[serde(default)]
    zoomed: Option<PaneId>,
}

impl Serialize for SplitTree {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        TreeWire {
            nodes: self.nodes.clone(),
            zoomed: self.zoomed,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SplitTree {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = TreeWire::deserialize(deserializer)?;
        SplitTree::from_nodes(wire.nodes, wire.zoomed).map_err(serde::de::Error::custom)
    }
}

impl SplitTree {
    /// A tree from a node array read from outside, checked: every node reachable from the root
    /// exactly once, every pane distinct, at most [`MOST_PANES`] panes.
    pub fn from_nodes(nodes: Vec<Node>, zoomed: Option<PaneId>) -> Result<SplitTree, CoreError> {
        if nodes.is_empty() {
            return Err(CoreError::BadTree("no panes"));
        }
        if nodes.len() >= 2 * MOST_PANES {
            return Err(CoreError::BadTree("too many panes"));
        }
        check_one_tree(&nodes)?;
        let raw = SplitTree {
            nodes,
            zoomed: None,
        };
        let mut canonical = Vec::with_capacity(raw.nodes.len());
        raw.copy_with(Handle::ROOT, &mut canonical, &mut |pane, out| {
            out.push(Node::Leaf(pane));
        });
        let tree = SplitTree {
            nodes: canonical,
            zoomed: None,
        };
        let zoomed = zoomed.filter(|pane| tree.contains(*pane) && tree.is_split());
        Ok(SplitTree { zoomed, ..tree })
    }
}

/// Walks from the root without recursion: each handle in range and seen once, every node seen,
/// no pane twice.
fn check_one_tree(nodes: &[Node]) -> Result<(), CoreError> {
    let mut seen = vec![false; nodes.len()];
    let mut panes = BTreeSet::new();
    let mut stack = vec![Handle::ROOT];
    while let Some(at) = stack.pop() {
        let slot = seen
            .get_mut(at.index())
            .ok_or(CoreError::BadTree("a handle points past the end"))?;
        if *slot {
            return Err(CoreError::BadTree("a node is reached twice"));
        }
        *slot = true;
        match nodes[at.index()] {
            Node::Leaf(pane) => {
                if !panes.insert(pane) {
                    return Err(CoreError::BadTree("a pane appears twice"));
                }
            }
            Node::Split { first, second, .. } => {
                stack.push(second);
                stack.push(first);
            }
        }
    }
    if seen.iter().all(|reached| *reached) {
        Ok(())
    } else {
        Err(CoreError::BadTree("a node is not reachable"))
    }
}
