//! The split tree, ported from Ghostty's `src/datastruct/split_tree.zig` (MIT, Copyright (c)
//! 2024 Mitchell Hashimoto, Ghostty contributors): a flat node array with handles, each split's
//! ratio relative to its parent, and every operation returning a new tree. The port keeps
//! Ghostty's behaviour (split, remove, resize by a window-relative delta, equalize by per-axis
//! weights, zoom, in-order and spatial goto over a normalised square) and changes its
//! representation: ratios are integer millionths, nodes stay in canonical pre-order, and the
//! zoom names a pane, so trees compare and store exactly.

mod goto;
mod graft;
#[cfg(test)]
mod graft_tests;
mod layout;
mod model;
mod ops;
mod step;
#[cfg(test)]
mod step_tests;
#[cfg(test)]
mod tests;
mod wire;

pub use layout::{PaneRect, SPATIAL_UNIT, WholePx};
pub use model::{Axis, Direction, Goto, Handle, MOST_PANES, Node, Share, SplitTree, WindowDelta};
pub use ops::Closed;
pub use step::{TreeAfter, TreeIn, TreeOut, TreeState, tree_step};
