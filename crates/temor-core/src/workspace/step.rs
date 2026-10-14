//! `workspace_step`: the one entry the window calls for tabs, panes and the recently closed
//! list. It composes `tree_step` (inside a tab), `tabs_step` (the list) and `closed_step`, and
//! tells the window what to spawn, snapshot, end and forget.

use super::model::Workspace;
use super::step_close::{close_pane, close_tab, reopen};
use super::step_move::{drop_tab, tear};
use super::step_types::{RefuseWhy, Spawn, Start, WorkspaceIn, WorkspaceOut};
use super::step_viewer::open_viewer;
use super::tabs::{TabSel, TabsIn, TabsState, tabs_step};
use crate::space::{Pin, Tab};
use crate::split::{
    Columns, Direction, SplitTree, TreeAfter, TreeIn, TreeOut, TreeState, split_fits, tree_step,
};
use crate::vocab::ids::{PaneId, TabId};
use crate::vocab::text::{Cwd, Relaunch, TabTitle};

/// The workspace after `input`, and what the window must do.
pub fn workspace_step(mut ws: Workspace, input: WorkspaceIn) -> (Workspace, Vec<WorkspaceOut>) {
    let before = ws.clone();
    let mut out = Vec::new();
    match input {
        WorkspaceIn::NewTab { cwd } => new_tab(&mut ws, cwd, &mut out),
        WorkspaceIn::Split { towards } => split(&mut ws, towards, None, &mut out),
        WorkspaceIn::SplitIn { towards, width } => split(&mut ws, towards, Some(width), &mut out),
        WorkspaceIn::ClosePane(pane) => match pane.or_else(|| front_pane(&ws)) {
            Some(pane) => close_pane(&mut ws, pane, &mut out),
            None => out.push(WorkspaceOut::Refused(RefuseWhy::NoFront)),
        },
        WorkspaceIn::CloseTab(tab) => close_tab(&mut ws, tab, &mut out),
        WorkspaceIn::Goto(to) => in_front(&mut ws, &mut out, TreeIn::Goto(to)),
        WorkspaceIn::ToggleZoom => {
            let zoomed = front_tab(&ws).is_some_and(|t| t.tree.zoomed().is_some());
            let input = if zoomed {
                TreeIn::Unzoom
            } else {
                match front_pane(&ws) {
                    Some(pane) => TreeIn::Zoom(pane),
                    None => {
                        out.push(WorkspaceOut::Refused(RefuseWhy::NoFront));
                        return (ws, out);
                    }
                }
            };
            in_front(&mut ws, &mut out, input);
        }
        WorkspaceIn::Equalize => in_front(&mut ws, &mut out, TreeIn::Equalize),
        WorkspaceIn::SetShare {
            before,
            after,
            share,
        } => in_front(
            &mut ws,
            &mut out,
            TreeIn::SetShare {
                before,
                after,
                share,
            },
        ),
        WorkspaceIn::Focus(pane) => focus(&mut ws, pane, &mut out),
        WorkspaceIn::DropTab {
            from,
            onto,
            towards,
        } => drop_tab(&mut ws, from, onto, towards, &mut out),
        WorkspaceIn::DropTabIn {
            from,
            onto,
            towards,
            width,
        } => match split_fits(width, towards) {
            true => drop_tab(&mut ws, from, onto, towards, &mut out),
            false => out.push(WorkspaceOut::Refused(RefuseWhy::TooNarrow)),
        },
        WorkspaceIn::Tear(pane) => tear(&mut ws, pane, &mut out),
        WorkspaceIn::Activity(pane) => {
            if let Some(tab) = ws.tab_of(pane) {
                let (tabs, _) = tabs_step(take(&mut ws.tabs), TabsIn::Unread(tab));
                ws.tabs = tabs;
            }
        }
        WorkspaceIn::Cwd { pane, cwd } => {
            if ws.tab_of(pane).is_some() {
                ws.cwds.insert(pane, cwd);
            }
        }
        WorkspaceIn::OpenViewer { file, from } => open_viewer(&mut ws, from, file, &mut out),
        WorkspaceIn::ViewerShows { pane, file } => {
            if let Some(shown) = ws.viewers.get_mut(&pane) {
                *shown = file;
            }
        }
        WorkspaceIn::Reopen => reopen(&mut ws, &mut out),
        WorkspaceIn::Tabs(TabsIn::Remove(_)) => {
            out.push(WorkspaceOut::Refused(RefuseWhy::UseCloseTab));
        }
        WorkspaceIn::Tabs(input) => {
            let (tabs, outs) = tabs_step(take(&mut ws.tabs), input);
            ws.tabs = tabs;
            out.extend(outs.into_iter().map(WorkspaceOut::Tab));
        }
    }
    if ws != before {
        out.push(WorkspaceOut::Changed);
    }
    (ws, out)
}

pub(super) fn take(tabs: &mut TabsState) -> TabsState {
    std::mem::take(tabs)
}

fn front_tab(ws: &Workspace) -> Option<&Tab> {
    ws.front().and_then(|id| ws.tabs.tab(id))
}

pub(super) fn front_pane(ws: &Workspace) -> Option<PaneId> {
    ws.front().and_then(|id| ws.focus_of(id))
}

/// The tree and focus of `tab`.
pub(super) fn tree_state(ws: &Workspace, tab: TabId) -> Option<TreeState> {
    let focus = ws.focus_of(tab)?;
    let t = ws.tabs.tab(tab)?;
    Some(TreeState {
        tree: t.tree.clone(),
        focus,
    })
}

/// Stores a tab's tree and focus.
pub(super) fn store_tree_state(ws: &mut Workspace, tab: TabId, state: TreeState) {
    if let Some(t) = ws.tabs.place.tabs.iter_mut().find(|t| t.id == tab) {
        t.tree = state.tree;
    }
    ws.focus.insert(tab, state.focus);
}

/// The next pane id, or none when the counter has no id left to hand out after it.
pub(super) fn mint_pane(ws: &mut Workspace) -> Option<PaneId> {
    let next = ws.next_pane.checked_add(1)?;
    let pane = PaneId(ws.next_pane);
    ws.next_pane = next;
    Some(pane)
}

/// The next tab id, or none when the counter has no id left to hand out after it.
pub(super) fn mint_tab(ws: &mut Workspace) -> Option<TabId> {
    let next = ws.next_tab.checked_add(1)?;
    let tab = TabId(ws.next_tab);
    ws.next_tab = next;
    Some(tab)
}

/// The relaunch command for a pane: a pinned tab's first pane runs its command again.
pub(super) fn relaunch_for(tab: &Tab, pane: PaneId) -> Option<Relaunch> {
    match &tab.pin {
        Pin::Pinned { relaunch } if tab.tree.panes().first() == Some(&pane) => {
            Some(relaunch.clone())
        }
        _ => None,
    }
}

pub(super) fn spawn_fresh(
    ws: &Workspace,
    tab: TabId,
    pane: PaneId,
    cwd: Option<Cwd>,
    out: &mut Vec<WorkspaceOut>,
) {
    let relaunch = ws.tabs.tab(tab).and_then(|t| relaunch_for(t, pane));
    out.push(WorkspaceOut::Spawn(Spawn {
        pane,
        tab,
        cwd,
        relaunch,
        start: Start::Fresh,
    }));
}

fn new_tab(ws: &mut Workspace, cwd: Option<Cwd>, out: &mut Vec<WorkspaceOut>) {
    // Both counters are checked before either moves, so a refusal changes nothing.
    if ws.next_tab.checked_add(1).is_none() || ws.next_pane.checked_add(1).is_none() {
        return out.push(WorkspaceOut::Refused(RefuseWhy::IdsExhausted));
    }
    let (Some(tab_id), Some(pane)) = (mint_tab(ws), mint_pane(ws)) else {
        return;
    };
    ws.tabs.place.tabs.push(Tab {
        id: tab_id,
        title: TabTitle("Shell".to_owned()),
        pin: Pin::Loose,
        folder: None,
        tree: SplitTree::single(pane),
    });
    ws.focus.insert(tab_id, pane);
    if let Some(cwd) = cwd.clone() {
        ws.cwds.insert(pane, cwd);
    }
    let (tabs, _) = tabs_step(take(&mut ws.tabs), TabsIn::Select(TabSel::Id(tab_id)));
    ws.tabs = tabs;
    spawn_fresh(ws, tab_id, pane, cwd, out);
    out.push(WorkspaceOut::Focused(pane));
}

fn split(
    ws: &mut Workspace,
    towards: Direction,
    width: Option<Columns>,
    out: &mut Vec<WorkspaceOut>,
) {
    if width.is_some_and(|width| !split_fits(width, towards)) {
        return out.push(WorkspaceOut::Refused(RefuseWhy::TooNarrow));
    }
    let Some(tab) = ws.front() else {
        return out.push(WorkspaceOut::Refused(RefuseWhy::NoFront));
    };
    let Some(state) = tree_state(ws, tab) else {
        return out.push(WorkspaceOut::Refused(RefuseWhy::NoFront));
    };
    let Some(next_pane) = ws.next_pane.checked_add(1) else {
        return out.push(WorkspaceOut::Refused(RefuseWhy::IdsExhausted));
    };
    let new = PaneId(ws.next_pane);
    let origin = state.focus;
    let (after, outs) = tree_step(state, TreeIn::Split { towards, new });
    if outs.contains(&TreeOut::SplitRefused) {
        return out.push(WorkspaceOut::Refused(RefuseWhy::TabFull));
    }
    if let TreeAfter::Open(next) = after {
        ws.next_pane = next_pane;
        store_tree_state(ws, tab, next);
        // The new pane starts where the one it came from is.
        let cwd = ws.cwds.get(&origin).cloned();
        if let Some(cwd) = cwd.clone() {
            ws.cwds.insert(new, cwd);
        }
        spawn_fresh(ws, tab, new, cwd, out);
        out.push(WorkspaceOut::Focused(new));
    }
}

pub(super) fn focus(ws: &mut Workspace, pane: PaneId, out: &mut Vec<WorkspaceOut>) {
    let Some(tab) = ws.tab_of(pane) else {
        return out.push(WorkspaceOut::Refused(RefuseWhy::NoSuchPane(pane)));
    };
    if ws.front() != Some(tab) {
        let (tabs, _) = tabs_step(take(&mut ws.tabs), TabsIn::Select(TabSel::Id(tab)));
        ws.tabs = tabs;
    }
    if let Some(state) = tree_state(ws, tab) {
        let (after, _) = tree_step(state, TreeIn::Focus(pane));
        if let TreeAfter::Open(next) = after {
            store_tree_state(ws, tab, next);
        }
    }
    out.push(WorkspaceOut::Focused(pane));
}

fn in_front(ws: &mut Workspace, out: &mut Vec<WorkspaceOut>, input: TreeIn) {
    let Some(tab) = ws.front() else {
        return out.push(WorkspaceOut::Refused(RefuseWhy::NoFront));
    };
    let Some(state) = tree_state(ws, tab) else {
        return;
    };
    if let (TreeAfter::Open(next), _) = tree_step(state, input) {
        let focus = next.focus;
        store_tree_state(ws, tab, next);
        out.push(WorkspaceOut::Focused(focus));
    }
}
