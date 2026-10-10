//! Opening a file in a viewer pane (PLAN §10): a split to the right of the pane it came from, or
//! the viewer the tab already has.

use super::model::Workspace;
use super::step::{focus, front_pane, store_tree_state, tree_state};
use super::step_types::{RefuseWhy, ViewerOpen, WorkspaceOut};
use crate::split::{Direction, TreeAfter, TreeIn, TreeOut, tree_step};
use crate::vocab::ids::PaneId;
use crate::vocab::text::ViewerRef;

pub(super) fn open_viewer(
    ws: &mut Workspace,
    from: Option<PaneId>,
    file: ViewerRef,
    out: &mut Vec<WorkspaceOut>,
) {
    if file.as_str().is_empty() {
        return out.push(WorkspaceOut::Refused(RefuseWhy::NoFile));
    }
    let Some(origin) = from.or_else(|| front_pane(ws)) else {
        return out.push(WorkspaceOut::Refused(RefuseWhy::NoFront));
    };
    let Some(tab) = ws.tab_of(origin) else {
        return out.push(WorkspaceOut::Refused(RefuseWhy::NoSuchPane(origin)));
    };
    // A tab shows one viewer: the next file replaces the last, as a preview pane does.
    let held = ws
        .tabs
        .tab(tab)
        .map(|t| t.tree.panes())
        .unwrap_or_default()
        .into_iter()
        .find(|pane| ws.viewers.contains_key(pane));
    if let Some(pane) = held {
        ws.viewers.insert(pane, file.clone());
        out.push(WorkspaceOut::Viewer(ViewerOpen {
            pane,
            tab,
            file,
            from: Some(origin),
        }));
        return focus(ws, pane, out);
    }
    let Some(mut state) = tree_state(ws, tab) else {
        return out.push(WorkspaceOut::Refused(RefuseWhy::NoFront));
    };
    // The split is of the pane the file came from, whichever pane has the keys.
    state.focus = origin;
    let Some(next_pane) = ws.next_pane.checked_add(1) else {
        return out.push(WorkspaceOut::Refused(RefuseWhy::IdsExhausted));
    };
    let new = PaneId(ws.next_pane);
    let (after, outs) = tree_step(
        state,
        TreeIn::Split {
            towards: Direction::Right,
            new,
        },
    );
    if outs.contains(&TreeOut::SplitRefused) {
        return out.push(WorkspaceOut::Refused(RefuseWhy::TabFull));
    }
    if let TreeAfter::Open(next) = after {
        ws.next_pane = next_pane;
        store_tree_state(ws, tab, next);
        ws.viewers.insert(new, file.clone());
        out.push(WorkspaceOut::Viewer(ViewerOpen {
            pane: new,
            tab,
            file,
            from: Some(origin),
        }));
        out.push(WorkspaceOut::Focused(new));
    }
}
