//! `workspace_step`: the one entry the window calls for tabs, panes and the recently closed
//! list. It composes `tree_step` (inside a tab), `tabs_step` (the list) and `closed_step`, and
//! tells the window what to spawn, snapshot, end and forget.

use super::closed::{ClosedIn, ClosedOut, ClosedState, ClosedTab, closed_step};
use super::model::Workspace;
use super::tabs::{TabSel, TabsIn, TabsOut, tabs_step};
use crate::error::CoreError;
use crate::space::{Pin, Tab};
use crate::split::{Direction, Goto, SplitTree, TreeAfter, TreeIn, TreeOut, TreeState, tree_step};
use crate::vocab::ids::{PaneId, TabId};
use crate::vocab::text::{Cwd, Relaunch, TabTitle};

/// Where a new pane's program starts from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Start {
    /// A fresh shell (or the tab's pinned command).
    Fresh,
    /// Its stored scrollback, from a pane that was closed.
    Scrollback(PaneId),
}

/// A pane the window must start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spawn {
    /// The pane.
    pub pane: PaneId,
    /// The tab it is in.
    pub tab: TabId,
    /// The folder to start in.
    pub cwd: Option<Cwd>,
    /// The command a pinned tab runs again (its first pane only).
    pub relaunch: Option<Relaunch>,
    /// Where its program starts from.
    pub start: Start,
}

/// What the workspace is asked to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceIn {
    /// Open a plain tab with one shell, in `cwd` when given.
    NewTab { cwd: Option<Cwd> },
    /// Split the focused pane of the tab in front, and focus the new pane.
    Split { towards: Direction },
    /// Close a pane (the focused one when none is named). The last pane closes the tab.
    ClosePane(Option<PaneId>),
    /// Close a tab with all its panes; it is kept as recently closed.
    CloseTab(TabId),
    /// Move focus within the tab in front.
    Goto(Goto),
    /// Show the focused pane over the whole tab, or back to the split (toggles).
    ToggleZoom,
    /// Even out the splits of the tab in front.
    Equalize,
    /// Focus a pane wherever it is, bringing its tab to the front.
    Focus(PaneId),
    /// Drop a tab onto a pane of another tab, splitting that pane towards `towards`.
    DropTab {
        /// The tab dragged.
        from: TabId,
        /// The pane it is dropped on.
        onto: PaneId,
        /// The side of `onto` it lands on.
        towards: Direction,
    },
    /// Tear a pane out of its tab into a tab of its own.
    Tear(PaneId),
    /// A command ended in a pane: its tab is marked unread unless it is in front.
    Activity(PaneId),
    /// The pane's working folder changed (OSC 7).
    Cwd { pane: PaneId, cwd: Cwd },
    /// Take back the newest recently closed tab.
    Reopen,
    /// A tab-list request (select, pin, folders, rename). Removing tabs goes through `CloseTab`.
    Tabs(TabsIn),
}

/// What the workspace tells the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceOut {
    /// Start this pane.
    Spawn(Spawn),
    /// Take this pane's scrollback now, before it ends.
    Snapshot(PaneId),
    /// End this pane's program and drop the pane.
    End(PaneId),
    /// These panes left the recently closed list: drop their stored scrollback.
    Forgotten(Vec<PaneId>),
    /// This pane has keys now.
    Focused(PaneId),
    /// A tab-list answer that the window does not act on itself.
    Tab(TabsOut),
    /// The request was not carried out.
    Refused(RefuseWhy),
    /// The state changed: the window saves the session (debounced).
    Changed,
}

/// Why a workspace request was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefuseWhy {
    /// The tab already holds the most panes it can.
    TabFull,
    /// The pane is not in any tab.
    NoSuchPane(PaneId),
    /// The tab is not in the list.
    NoSuchTab(TabId),
    /// A tab cannot be dropped onto its own pane.
    SameTab,
    /// A pinned tab keeps its pin; it is not merged into another tab.
    PinnedTab(TabId),
    /// A tab's only pane cannot be torn off; closing the tab is the way.
    OnlyPane(PaneId),
    /// No tab has been closed to bring back.
    NothingClosed,
    /// The request needs a tab-list step that does not exist here (removal).
    UseCloseTab,
    /// There is no tab in front.
    NoFront,
}

/// The workspace after `input`, and what the window must do.
pub fn workspace_step(mut ws: Workspace, input: WorkspaceIn) -> (Workspace, Vec<WorkspaceOut>) {
    let before = ws.clone();
    let mut out = Vec::new();
    match input {
        WorkspaceIn::NewTab { cwd } => new_tab(&mut ws, cwd, &mut out),
        WorkspaceIn::Split { towards } => split(&mut ws, towards, &mut out),
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
        WorkspaceIn::Focus(pane) => focus(&mut ws, pane, &mut out),
        WorkspaceIn::DropTab {
            from,
            onto,
            towards,
        } => drop_tab(&mut ws, from, onto, towards, &mut out),
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

fn take(tabs: &mut super::tabs::TabsState) -> super::tabs::TabsState {
    std::mem::take(tabs)
}

fn front_tab(ws: &Workspace) -> Option<&Tab> {
    ws.front().and_then(|id| ws.tabs.tab(id))
}

fn front_pane(ws: &Workspace) -> Option<PaneId> {
    ws.front().and_then(|id| ws.focus_of(id))
}

/// The tree and focus of `tab`.
fn tree_state(ws: &Workspace, tab: TabId) -> Option<TreeState> {
    let focus = ws.focus_of(tab)?;
    let t = ws.tabs.tab(tab)?;
    Some(TreeState {
        tree: t.tree.clone(),
        focus,
    })
}

/// Stores a tab's tree and focus.
fn store(ws: &mut Workspace, tab: TabId, state: TreeState) {
    if let Some(t) = ws.tabs.place.tabs.iter_mut().find(|t| t.id == tab) {
        t.tree = state.tree;
    }
    ws.focus.insert(tab, state.focus);
}

fn mint_pane(ws: &mut Workspace) -> PaneId {
    let pane = PaneId(ws.next_pane);
    ws.next_pane += 1;
    pane
}

fn mint_tab(ws: &mut Workspace) -> TabId {
    let tab = TabId(ws.next_tab);
    ws.next_tab += 1;
    tab
}

/// The relaunch command for a pane: a pinned tab's first pane runs its command again.
fn relaunch_for(tab: &Tab, pane: PaneId) -> Option<Relaunch> {
    match &tab.pin {
        Pin::Pinned { relaunch } if tab.tree.panes().first() == Some(&pane) => {
            Some(relaunch.clone())
        }
        _ => None,
    }
}

fn spawn_fresh(
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
    let tab_id = mint_tab(ws);
    let pane = mint_pane(ws);
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

fn split(ws: &mut Workspace, towards: Direction, out: &mut Vec<WorkspaceOut>) {
    let Some(tab) = ws.front() else {
        return out.push(WorkspaceOut::Refused(RefuseWhy::NoFront));
    };
    let Some(state) = tree_state(ws, tab) else {
        return out.push(WorkspaceOut::Refused(RefuseWhy::NoFront));
    };
    let new = PaneId(ws.next_pane);
    let origin = state.focus;
    let (after, outs) = tree_step(state, TreeIn::Split { towards, new });
    if outs.contains(&TreeOut::SplitRefused) {
        return out.push(WorkspaceOut::Refused(RefuseWhy::TabFull));
    }
    if let TreeAfter::Open(next) = after {
        ws.next_pane += 1;
        store(ws, tab, next);
        // The new pane starts where the one it came from is.
        let cwd = ws.cwds.get(&origin).cloned();
        if let Some(cwd) = cwd.clone() {
            ws.cwds.insert(new, cwd);
        }
        spawn_fresh(ws, tab, new, cwd, out);
        out.push(WorkspaceOut::Focused(new));
    }
}

fn focus(ws: &mut Workspace, pane: PaneId, out: &mut Vec<WorkspaceOut>) {
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
            store(ws, tab, next);
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
        store(ws, tab, next);
        out.push(WorkspaceOut::Focused(focus));
    }
}

fn close_pane(ws: &mut Workspace, pane: PaneId, out: &mut Vec<WorkspaceOut>) {
    let Some(tab) = ws.tab_of(pane) else {
        return out.push(WorkspaceOut::Refused(RefuseWhy::NoSuchPane(pane)));
    };
    let Some(state) = tree_state(ws, tab) else {
        return;
    };
    match tree_step(state, TreeIn::Close(pane)).0 {
        TreeAfter::Open(next) => {
            let focus = next.focus;
            store(ws, tab, next);
            ws.cwds.remove(&pane);
            out.push(WorkspaceOut::Snapshot(pane));
            out.push(WorkspaceOut::End(pane));
            out.push(WorkspaceOut::Focused(focus));
        }
        TreeAfter::Emptied => close_tab(ws, tab, out),
    }
}

fn close_tab(ws: &mut Workspace, tab: TabId, out: &mut Vec<WorkspaceOut>) {
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

fn reopen(ws: &mut Workspace, out: &mut Vec<WorkspaceOut>) {
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

fn tear(ws: &mut Workspace, pane: PaneId, out: &mut Vec<WorkspaceOut>) {
    let Some(tab) = ws.tab_of(pane) else {
        return out.push(WorkspaceOut::Refused(RefuseWhy::NoSuchPane(pane)));
    };
    let Some(source) = ws.tabs.tab(tab).cloned() else {
        return;
    };
    match source.tree.tear(pane) {
        Err(CoreError::LastPane) => out.push(WorkspaceOut::Refused(RefuseWhy::OnlyPane(pane))),
        Err(_) => out.push(WorkspaceOut::Refused(RefuseWhy::NoSuchPane(pane))),
        Ok((rest, alone)) => {
            let rest_focus = if ws.focus_of(tab) == Some(pane) {
                rest.panes().first().copied()
            } else {
                ws.focus_of(tab)
            };
            if let (Some(focus), Some(t)) = (
                rest_focus,
                ws.tabs.place.tabs.iter_mut().find(|t| t.id == tab),
            ) {
                t.tree = rest;
                ws.focus.insert(tab, focus);
            }
            let new_id = mint_tab(ws);
            ws.tabs.place.tabs.push(Tab {
                id: new_id,
                title: source.title,
                pin: Pin::Loose,
                folder: source.folder,
                tree: alone,
            });
            ws.focus.insert(new_id, pane);
            let (tabs, _) = tabs_step(take(&mut ws.tabs), TabsIn::Select(TabSel::Id(new_id)));
            ws.tabs = tabs;
            out.push(WorkspaceOut::Focused(pane));
        }
    }
}

fn drop_tab(
    ws: &mut Workspace,
    from: TabId,
    onto: PaneId,
    towards: Direction,
    out: &mut Vec<WorkspaceOut>,
) {
    let Some(target) = ws.tab_of(onto) else {
        return out.push(WorkspaceOut::Refused(RefuseWhy::NoSuchPane(onto)));
    };
    if ws.tabs.tab(from).is_none() {
        return out.push(WorkspaceOut::Refused(RefuseWhy::NoSuchTab(from)));
    }
    if target == from {
        return out.push(WorkspaceOut::Refused(RefuseWhy::SameTab));
    }
    if matches!(ws.tabs.tab(from).map(|t| &t.pin), Some(Pin::Pinned { .. })) {
        return out.push(WorkspaceOut::Refused(RefuseWhy::PinnedTab(from)));
    }
    let (Some(dragged), Some(host)) = (
        tree_state(ws, from),
        ws.tabs.tab(target).map(|t| t.tree.clone()),
    ) else {
        return;
    };
    let Ok(merged) = host.graft(onto, &dragged.tree, towards) else {
        return out.push(WorkspaceOut::Refused(RefuseWhy::TabFull));
    };
    let focus = dragged.focus;
    if let Some(t) = ws.tabs.place.tabs.iter_mut().find(|t| t.id == target) {
        t.tree = merged;
    }
    ws.focus.insert(target, focus);
    ws.focus.remove(&from);
    let (tabs, _) = tabs_step(take(&mut ws.tabs), TabsIn::Remove(from));
    ws.tabs = tabs;
    let (tabs, _) = tabs_step(take(&mut ws.tabs), TabsIn::Select(TabSel::Id(target)));
    ws.tabs = tabs;
    out.push(WorkspaceOut::Focused(focus));
}
