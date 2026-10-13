//! `tree_step`: a tab's split tree and its focused pane, moved by the window's split commands.

use super::model::{Axis, Direction, Goto, Share, SplitTree, WindowDelta};
use super::ops::Closed;
use crate::vocab::ids::PaneId;

/// A tab's panes and which one has focus. The focused pane is always in the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeState {
    /// The panes.
    pub tree: SplitTree,
    /// The pane keys go to.
    pub focus: PaneId,
}

impl TreeState {
    /// A tab with one pane, focused.
    pub fn single(pane: PaneId) -> TreeState {
        TreeState {
            tree: SplitTree::single(pane),
            focus: pane,
        }
    }
}

/// A split command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeIn {
    /// Split the focused pane, putting `new` (an id the caller allocated) `towards` it.
    Split {
        /// Where the new pane goes.
        towards: Direction,
        /// The new pane.
        new: PaneId,
    },
    /// Close a pane.
    Close(PaneId),
    /// Move the divider nearest the focused pane along `axis` by part of the window.
    Resize {
        /// Which dividers.
        axis: Axis,
        /// How far, as part of the whole window.
        delta: WindowDelta,
    },
    /// Even out every split.
    Equalize,
    /// Give the split between two neighbouring panes this share: a divider the person dragged.
    SetShare {
        /// The pane before the divider.
        before: PaneId,
        /// The pane after it.
        after: PaneId,
        /// The first child's part of the split.
        share: Share,
    },
    /// Show `pane` over the whole tab, and focus it.
    Zoom(PaneId),
    /// Show every pane again.
    Unzoom,
    /// Move focus.
    Goto(Goto),
    /// Focus a pane the person clicked.
    Focus(PaneId),
}

/// What the tab wants done after a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeOut {
    /// Keys now go to this pane.
    Focused(PaneId),
    /// The tab holds the most panes it can; the split was not made.
    SplitRefused,
}

/// The tab after a step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeAfter {
    /// The tab is still open.
    Open(TreeState),
    /// Its last pane closed; the tab closes.
    Emptied,
}

/// The tab after `input`. Commands naming a pane the tree does not hold leave it unchanged,
/// because they come from a view of the tab that is already stale.
pub fn tree_step(state: TreeState, input: TreeIn) -> (TreeAfter, Vec<TreeOut>) {
    match input {
        TreeIn::Split { towards, new } => match state.tree.split(state.focus, new, towards) {
            Ok(tree) => focused(TreeState { tree, focus: new }),
            Err(crate::error::CoreError::TreeFull) => {
                (TreeAfter::Open(state), vec![TreeOut::SplitRefused])
            }
            Err(_) => unchanged(state),
        },
        TreeIn::Close(pane) => close(state, pane),
        TreeIn::Resize { axis, delta } => match state.tree.resize(state.focus, axis, delta) {
            Ok(tree) => unchanged(TreeState { tree, ..state }),
            Err(_) => unchanged(state),
        },
        TreeIn::Equalize => unchanged(TreeState {
            tree: state.tree.equalize(),
            ..state
        }),
        TreeIn::SetShare {
            before,
            after,
            share,
        } => unchanged(TreeState {
            tree: state.tree.set_share(before, after, share),
            ..state
        }),
        TreeIn::Zoom(pane) => match state.tree.zoom(pane) {
            Ok(tree) => move_focus(state.focus, TreeState { tree, focus: pane }),
            Err(_) => unchanged(state),
        },
        TreeIn::Unzoom => unchanged(TreeState {
            tree: state.tree.unzoom(),
            ..state
        }),
        TreeIn::Goto(to) => match state.tree.goto(state.focus, to) {
            Some(pane) => move_focus(
                state.focus,
                TreeState {
                    tree: state.tree.unzoom(),
                    focus: pane,
                },
            ),
            None => unchanged(state),
        },
        TreeIn::Focus(pane) if state.tree.contains(pane) && pane != state.focus => {
            // Keys must never go to a pane the zoom hides.
            let tree = match state.tree.zoomed() {
                Some(zoomed) if zoomed != pane => state.tree.unzoom(),
                _ => state.tree,
            };
            focused(TreeState { tree, focus: pane })
        }
        TreeIn::Focus(_) => unchanged(state),
    }
}

/// Closing the focused pane moves focus to the pane before it in reading order, else the one
/// after.
fn close(state: TreeState, pane: PaneId) -> (TreeAfter, Vec<TreeOut>) {
    let heir = state
        .tree
        .goto(pane, Goto::Previous)
        .or_else(|| state.tree.goto(pane, Goto::Next));
    match state.tree.remove(pane) {
        Ok(Closed::Last) => (TreeAfter::Emptied, vec![]),
        Ok(Closed::Remaining(tree)) if pane == state.focus => match heir {
            Some(focus) => focused(TreeState { tree, focus }),
            None => unchanged(state),
        },
        Ok(Closed::Remaining(tree)) => unchanged(TreeState { tree, ..state }),
        Err(_) => unchanged(state),
    }
}

fn unchanged(state: TreeState) -> (TreeAfter, Vec<TreeOut>) {
    (TreeAfter::Open(state), vec![])
}

fn focused(state: TreeState) -> (TreeAfter, Vec<TreeOut>) {
    let out = vec![TreeOut::Focused(state.focus)];
    (TreeAfter::Open(state), out)
}

fn move_focus(before: PaneId, state: TreeState) -> (TreeAfter, Vec<TreeOut>) {
    if state.focus == before {
        unchanged(state)
    } else {
        focused(state)
    }
}
