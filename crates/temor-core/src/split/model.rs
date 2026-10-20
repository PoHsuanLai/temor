//! The split tree's data: a flat node array addressed by handles, each split's share relative
//! to its parent, and the pane (if any) that is zoomed.

use crate::vocab::ids::PaneId;
use ds_core::word::Word;
use serde::{Deserialize, Serialize};

/// The most panes one tab holds. Bounds the tree's depth, so walking a stored tree can never
/// exhaust the stack, and keeps every handle inside a `u16`.
pub const MOST_PANES: usize = 128;

/// A node's place in [`SplitTree::nodes`]. The root is always 0, and the array is kept in
/// pre-order, so a split's first child is the handle after its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Handle(pub u16);

impl Handle {
    /// The root node.
    pub const ROOT: Handle = Handle(0);

    /// The handle as an index into the node array.
    pub fn index(self) -> usize {
        usize::from(self.0)
    }
}

/// How a split lays out its two children.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum Axis {
    /// Side by side: the first child on the left, a vertical divider between them.
    Row,
    /// Stacked: the first child on top, a horizontal divider between them.
    Column,
}

/// A direction on screen: where a new pane goes, or where focus moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum Direction {
    /// Towards the left edge.
    Left,
    /// Towards the right edge.
    Right,
    /// Towards the top edge.
    Up,
    /// Towards the bottom edge.
    Down,
}

impl Direction {
    /// The axis a split in this direction lays out along.
    pub fn axis(self) -> Axis {
        match self {
            Direction::Left | Direction::Right => Axis::Row,
            Direction::Up | Direction::Down => Axis::Column,
        }
    }

    /// Whether this direction points at a split's first child (left or top).
    pub fn is_towards_first(self) -> bool {
        matches!(self, Direction::Left | Direction::Up)
    }

    /// The direction back.
    pub fn opposite(self) -> Direction {
        match self {
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
        }
    }
}

/// The part of its parent a split's first child takes, in millionths. Integer, so a tree is
/// `Eq` and stores exactly. Always held between [`Share::LEAST`] and `WHOLE - LEAST`, so no pane
/// is ever squeezed to nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(from = "u32", into = "u32")]
pub struct Share(u32);

impl Share {
    /// All of the parent.
    pub const WHOLE: u32 = 1_000_000;
    /// The least either child takes: half a percent. Below `1 / MOST_PANES`, so equalizing any
    /// tree a tab can hold (at worst 1 pane against 127) is never clamped.
    pub const LEAST: u32 = 5_000;
    /// An even split.
    pub const HALF: Share = Share(Share::WHOLE / 2);

    /// `millionths`, held to the allowed range.
    pub fn new(millionths: i64) -> Share {
        let least = i64::from(Share::LEAST);
        let most = i64::from(Share::WHOLE - Share::LEAST);
        // The clamp keeps the value inside `LEAST..=WHOLE - LEAST`, which fits a u32.
        Share(u32::try_from(millionths.clamp(least, most)).unwrap_or(Share::HALF.0))
    }

    /// The share in millionths.
    pub fn millionths(self) -> u32 {
        self.0
    }

    /// The first child's part of `extent`, rounded down; the second child takes the rest, so the
    /// two always add up to `extent` exactly.
    pub fn first_part(self, extent: u64) -> u64 {
        extent.saturating_mul(u64::from(self.0)) / u64::from(Share::WHOLE)
    }
}

impl From<u32> for Share {
    fn from(millionths: u32) -> Share {
        Share::new(i64::from(millionths))
    }
}

impl From<Share> for u32 {
    fn from(share: Share) -> u32 {
        share.0
    }
}

/// A change to a divider, in millionths of the whole window along the split's axis: positive
/// moves it right or down. A keyboard resize says "10% of the window" whatever the split's size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowDelta(pub i32);

/// One node: a pane, or a split of two nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum Node {
    /// A pane.
    Leaf(PaneId),
    /// Two nodes side by side or stacked.
    Split {
        /// How the two children are laid out.
        axis: Axis,
        /// The first child's share of this split.
        ratio: Share,
        /// The left or top child.
        first: Handle,
        /// The right or bottom child.
        second: Handle,
    },
}

/// Where to move focus from a pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Goto {
    /// The pane before, in reading order; none from the first.
    Previous,
    /// The pane after; none from the last.
    Next,
    /// The pane before, wrapping from the first to the last.
    PreviousWrapped,
    /// The pane after, wrapping from the last to the first.
    NextWrapped,
    /// The nearest pane on screen in a direction, wrapping round the window's edge.
    Spatial(Direction),
}

/// The panes of one tab, split and nested. Ported from Ghostty's `split_tree.zig`: a flat node
/// array with handles instead of pointers, ratios relative to the parent, and every operation
/// returning a new tree. Never empty: closing the last pane is reported, not represented.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SplitTree {
    pub(super) nodes: Vec<Node>,
    pub(super) zoomed: Option<PaneId>,
}

impl SplitTree {
    /// A tree of one pane.
    pub fn single(pane: PaneId) -> SplitTree {
        SplitTree {
            nodes: vec![Node::Leaf(pane)],
            zoomed: None,
        }
    }

    /// Every node, root first, in pre-order.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// The node at `handle`, if there is one.
    pub fn node(&self, handle: Handle) -> Option<Node> {
        self.nodes.get(handle.index()).copied()
    }

    /// The pane shown over the whole tab, if one is zoomed.
    pub fn zoomed(&self) -> Option<PaneId> {
        self.zoomed
    }

    /// The panes in the order the tree nests them. For reading order on screen, which is what a
    /// person sees and what moving focus follows, use [`SplitTree::reading_order`].
    pub fn panes(&self) -> Vec<PaneId> {
        self.nodes
            .iter()
            .filter_map(|node| match node {
                Node::Leaf(pane) => Some(*pane),
                Node::Split { .. } => None,
            })
            .collect()
    }

    /// Whether `pane` is in this tree.
    pub fn contains(&self, pane: PaneId) -> bool {
        self.handle_of(pane).is_some()
    }

    /// Whether the tab is split at all.
    pub fn is_split(&self) -> bool {
        matches!(self.nodes.first(), Some(Node::Split { .. }))
    }

    /// The handle of `pane`'s leaf.
    pub fn handle_of(&self, pane: PaneId) -> Option<Handle> {
        self.nodes
            .iter()
            .position(|node| *node == Node::Leaf(pane))
            .and_then(|index| u16::try_from(index).ok())
            .map(Handle)
    }
}
