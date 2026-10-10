//! The tab in front as its tree of panes (PLAN §7): a leaf is a terminal pane, a split is one of
//! quire's `SplitView`s with the axis of its node and the node's share as the first pane's
//! starting size. Only this tab is drawn, so only its panes are laid out and painted.
//!
//! quire v0.3.1's `SplitView` takes a starting share but does not report a finished drag, so a
//! divider the person drags is not written back to the tree: it stays where it was dragged until
//! the tab's structure changes (a split, a close), when the tree's shares apply again. A quire ask.

use std::time::Instant;

use dioxus::prelude::*;
use ds::components::chrome::split_view::model::{
    PaneAt, PaneSpec, Share as ViewShare, SplitAxis, SplitPane, UnfocusedPanes,
};
use ds::components::chrome::split_view::view::SplitView;
use temor_core::split::{Axis, Handle, Node, Share, SplitTree};
use temor_core::vocab::ids::PaneId;

use super::pane_view::PaneView;
use super::viewer_pane::ViewerView;
use crate::hub::Desk;

/// The panes of the tab in front. `focus` is the pane that has the keys.
#[component]
#[allow(non_snake_case)]
pub fn TreeView(tree: SplitTree, focus: PaneId, desk: Desk, started: Instant) -> Element {
    match tree.zoomed() {
        Some(pane) => rsx! {
            Leaf { key: "{pane.0}", pane, desk, started }
        },
        None => rsx! {
            TreeNode { tree, at: Handle::ROOT, focus, desk, started }
        },
    }
}

/// One node: a pane, or a split of two nodes.
#[component]
#[allow(non_snake_case)]
fn TreeNode(tree: SplitTree, at: Handle, focus: PaneId, desk: Desk, started: Instant) -> Element {
    match tree.node(at) {
        Some(Node::Leaf(pane)) => rsx! {
            Leaf { key: "{pane.0}", pane, desk, started }
        },
        Some(Node::Split {
            axis,
            ratio,
            first,
            second,
        }) => {
            let key = structure_key(&tree, at);
            let held = match panes_under(&tree, first).contains(&focus) {
                true => PaneAt::Pane(0),
                false => PaneAt::Rest,
            };
            let before = rsx! {
                TreeNode { tree: tree.clone(), at: first, focus, desk: desk.clone(), started }
            };
            rsx! {
                // The key changes with the node's panes, so a divider the person dragged is
                // forgotten when the structure under it changes.
                SplitView {
                    key: "{key}",
                    label: "Panes",
                    panes: vec![SplitPane::new(spec_of(ratio), before)],
                    axis: axis_of(axis),
                    unfocused: UnfocusedPanes::Dim,
                    focused: Some(held),
                    TreeNode { tree, at: second, focus, desk, started }
                }
            }
        }
        None => rsx! {},
    }
}

/// One tile: a terminal, or a viewer when the pane shows a file.
#[component]
#[allow(non_snake_case)]
fn Leaf(pane: PaneId, desk: Desk, started: Instant) -> Element {
    match desk.hub.is_viewer(pane) {
        true => rsx! {
            ViewerView { pane, desk }
        },
        false => rsx! {
            PaneView { pane, desk, started }
        },
    }
}

/// The panes at and under `at`, in tree order.
pub fn panes_under(tree: &SplitTree, at: Handle) -> Vec<PaneId> {
    match tree.node(at) {
        Some(Node::Leaf(pane)) => vec![pane],
        Some(Node::Split { first, second, .. }) => {
            let mut panes = panes_under(tree, first);
            panes.extend(panes_under(tree, second));
            panes
        }
        None => Vec::new(),
    }
}

/// A name for the panes under `at`: it changes when a pane joins or leaves them.
fn structure_key(tree: &SplitTree, at: Handle) -> String {
    panes_under(tree, at)
        .iter()
        .map(|pane| pane.0.to_string())
        .collect::<Vec<_>>()
        .join("-")
}

/// The first pane's size before anyone drags it: the node's share of the split.
fn spec_of(ratio: Share) -> PaneSpec {
    let part = ratio.millionths() as f32 / Share::WHOLE as f32;
    PaneSpec::sharing(ViewShare(part))
}

fn axis_of(axis: Axis) -> SplitAxis {
    match axis {
        Axis::Row => SplitAxis::Row,
        Axis::Column => SplitAxis::Column,
    }
}
