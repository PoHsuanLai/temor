//! Moving panes between tabs: tearing one off, and dropping a whole tab onto a pane.

use super::model::Workspace;
use super::step::{mint_tab, take, tree_state};
use super::step_types::{RefuseWhy, WorkspaceOut};
use super::tabs::{TabSel, TabsIn, tabs_step};
use crate::error::CoreError;
use crate::space::{Pin, Tab};
use crate::split::Direction;
use crate::vocab::ids::{PaneId, TabId};

pub(super) fn tear(ws: &mut Workspace, pane: PaneId, out: &mut Vec<WorkspaceOut>) {
    let Some(tab) = ws.tab_of(pane) else {
        return out.push(WorkspaceOut::Refused(RefuseWhy::NoSuchPane(pane)));
    };
    let Some(source) = ws.tabs.tab(tab).cloned() else {
        return out.push(WorkspaceOut::Refused(RefuseWhy::NoSuchTab(tab)));
    };
    // A pinned tab keeps its panes, as it is not merged into another tab either.
    if matches!(source.pin, Pin::Pinned { .. }) {
        return out.push(WorkspaceOut::Refused(RefuseWhy::PinnedTab(tab)));
    }
    let (rest, alone) = match source.tree.tear(pane) {
        Ok(parts) => parts,
        Err(CoreError::LastPane) => {
            return out.push(WorkspaceOut::Refused(RefuseWhy::OnlyPane(pane)));
        }
        Err(_) => return out.push(WorkspaceOut::Refused(RefuseWhy::NoSuchPane(pane))),
    };
    let Some(rest_focus) = (if ws.focus_of(tab) == Some(pane) {
        rest.panes().first().copied()
    } else {
        ws.focus_of(tab)
    }) else {
        return out.push(WorkspaceOut::Refused(RefuseWhy::NoSuchPane(pane)));
    };
    let Some(new_id) = mint_tab(ws) else {
        return out.push(WorkspaceOut::Refused(RefuseWhy::IdsExhausted));
    };
    if let Some(t) = ws.tabs.place.tabs.iter_mut().find(|t| t.id == tab) {
        t.tree = rest;
    }
    ws.focus.insert(tab, rest_focus);
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

pub(super) fn drop_tab(
    ws: &mut Workspace,
    from: TabId,
    onto: PaneId,
    towards: Direction,
    out: &mut Vec<WorkspaceOut>,
) {
    let Some(target) = ws.tab_of(onto) else {
        return out.push(WorkspaceOut::Refused(RefuseWhy::NoSuchPane(onto)));
    };
    let Some(dragged_tab) = ws.tabs.tab(from) else {
        return out.push(WorkspaceOut::Refused(RefuseWhy::NoSuchTab(from)));
    };
    if target == from {
        return out.push(WorkspaceOut::Refused(RefuseWhy::SameTab));
    }
    if matches!(dragged_tab.pin, Pin::Pinned { .. }) {
        return out.push(WorkspaceOut::Refused(RefuseWhy::PinnedTab(from)));
    }
    let (Some(dragged), Some(host)) = (
        tree_state(ws, from),
        ws.tabs.tab(target).map(|t| t.tree.clone()),
    ) else {
        // A tab whose recorded view cannot be read: the caller's picture of it is stale.
        return out.push(WorkspaceOut::Refused(RefuseWhy::NoSuchTab(from)));
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
