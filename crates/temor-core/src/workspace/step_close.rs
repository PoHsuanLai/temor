//! Closing panes and tabs, and bringing a closed tab back.

use super::closed::{ClosedIn, ClosedOut, ClosedState, ClosedTab, closed_step};
use super::model::Workspace;
use super::step::{relaunch_for, store_tree_state, take, tree_state};
use super::step_types::{RefuseWhy, Spawn, Start, WorkspaceOut};
use super::tabs::{TabSel, TabsIn, TabsOut, tabs_step};
use crate::split::{TreeAfter, TreeIn, tree_step};
use crate::vocab::ids::{PaneId, TabId};

pub(super) fn close_pane(ws: &mut Workspace, pane: PaneId, out: &mut Vec<WorkspaceOut>) {
    let Some(tab) = ws.tab_of(pane) else {
        return out.push(WorkspaceOut::Refused(RefuseWhy::NoSuchPane(pane)));
    };
    let Some(state) = tree_state(ws, tab) else {
        return;
    };
    match tree_step(state, TreeIn::Close(pane)).0 {
        TreeAfter::Open(next) => {
            let focus = next.focus;
            store_tree_state(ws, tab, next);
            ws.cwds.remove(&pane);
            out.push(WorkspaceOut::Snapshot(pane));
            out.push(WorkspaceOut::End(pane));
            out.push(WorkspaceOut::Focused(focus));
        }
        TreeAfter::Emptied => close_tab(ws, tab, out),
    }
}

pub(super) fn close_tab(ws: &mut Workspace, tab: TabId, out: &mut Vec<WorkspaceOut>) {
    let Some(closing) = ws.tabs.tab(tab).cloned() else {
        return out.push(WorkspaceOut::Refused(RefuseWhy::NoSuchTab(tab)));
    };
    let focus = ws.focus_of(tab).unwrap_or(PaneId(0));
    let panes = closing.tree.panes();
    for pane in &panes {
        out.push(WorkspaceOut::Snapshot(*pane));
    }
    for pane in &panes {
        out.push(WorkspaceOut::End(*pane));
    }
    let (tabs, tab_outs) = tabs_step(take(&mut ws.tabs), TabsIn::Remove(tab));
    ws.tabs = tabs;
    ws.focus.remove(&tab);
    for t in tab_outs {
        match t {
            TabsOut::Focused(next) => {
                let next_focus = ws.focus_of(next);
                out.push(WorkspaceOut::Tab(TabsOut::Focused(next)));
                if let Some(pane) = next_focus {
                    out.push(WorkspaceOut::Focused(pane));
                }
            }
            TabsOut::Removed(_) | TabsOut::Created(_) | TabsOut::Refused(_) => {}
        }
    }
    let closed = take_closed(ws);
    let (closed, closed_outs) = closed_step(
        closed,
        ClosedIn::Push {
            tab: closing,
            focus,
        },
    );
    ws.closed = closed;
    for c in closed_outs {
        if let ClosedOut::Evicted(gone) = c {
            for pane in &gone {
                ws.cwds.remove(pane);
            }
            out.push(WorkspaceOut::Forgotten(gone));
        }
    }
}

fn take_closed(ws: &mut Workspace) -> ClosedState {
    let keep = ws.closed.keep;
    std::mem::replace(&mut ws.closed, ClosedState::new(keep))
}

pub(super) fn reopen(ws: &mut Workspace, out: &mut Vec<WorkspaceOut>) {
    let closed = take_closed(ws);
    let (closed, closed_outs) = closed_step(closed, ClosedIn::Reopen);
    ws.closed = closed;
    match closed_outs.into_iter().next() {
        Some(ClosedOut::Reopened(ClosedTab { tab, focus, .. })) => {
            let id = tab.id;
            let panes = tab.tree.panes();
            ws.focus.insert(id, focus);
            ws.tabs.place.tabs.push(tab);
            let (tabs, _) = tabs_step(take(&mut ws.tabs), TabsIn::Select(TabSel::Id(id)));
            ws.tabs = tabs;
            for pane in &panes {
                let relaunch = ws.tabs.tab(id).and_then(|t| relaunch_for(t, *pane));
                out.push(WorkspaceOut::Spawn(Spawn {
                    pane: *pane,
                    tab: id,
                    cwd: ws.cwds.get(pane).cloned(),
                    relaunch,
                    start: Start::Scrollback(*pane),
                }));
            }
            out.push(WorkspaceOut::Focused(focus));
        }
        _ => out.push(WorkspaceOut::Refused(RefuseWhy::NothingClosed)),
    }
}
