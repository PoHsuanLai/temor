//! The sidebar's order (PLAN §7): pinned tabs first, then each folder's tabs (a folder's own,
//! then the folders in it, depth first), then the rest.
//! ⌘1 to ⌘9 count along the same order, so a key and a row always name the same tab.

use std::collections::BTreeSet;

use super::tabs::TabsState;
use crate::space::{Folder, Pin};
use crate::vocab::ids::{FolderId, TabId};

/// Where each tab sits in the sidebar. A tab that is pinned appears under pinned, whatever its
/// folder; a folder with no tabs is still listed (the UI may leave it out).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SidebarPlan {
    /// Pinned tabs, in list order.
    pub pinned: Vec<TabId>,
    /// Each folder with its unpinned tabs, depth first (a folder before the folders in it), each
    /// folder's `parent` the one it is listed under.
    pub folders: Vec<(Folder, Vec<TabId>)>,
    /// Unpinned tabs without a folder, in list order, then those whose folder is gone (so no
    /// tab is ever unreachable).
    pub loose: Vec<TabId>,
}

/// The sidebar's sections for `state`.
pub fn sidebar_plan(state: &TabsState) -> SidebarPlan {
    let is_pinned = |tab: &crate::space::Tab| matches!(tab.pin, Pin::Pinned { .. });
    let pinned = state
        .place
        .tabs
        .iter()
        .filter(|tab| is_pinned(tab))
        .map(|tab| tab.id)
        .collect();
    let folders = folder_order(&state.place.folders)
        .into_iter()
        .map(|folder| {
            let tabs = state
                .place
                .tabs
                .iter()
                .filter(|tab| !is_pinned(tab) && tab.folder == Some(folder.id))
                .map(|tab| tab.id)
                .collect();
            (folder, tabs)
        })
        .collect();
    let has_folder =
        |tab: &crate::space::Tab| tab.folder.is_some_and(|id| state.folder(id).is_some());
    let loose = state
        .place
        .tabs
        .iter()
        .filter(|tab| !is_pinned(tab) && tab.folder.is_none())
        .chain(
            state
                .place
                .tabs
                .iter()
                .filter(|tab| !is_pinned(tab) && tab.folder.is_some() && !has_folder(tab)),
        )
        .map(|tab| tab.id)
        .collect();
    SidebarPlan {
        pinned,
        folders,
        loose,
    }
}

/// The folders as the sidebar lists them: each, then the folders in it, depth first, in the
/// order they were made. A folder whose `parent` is gone, or that a hand-edited file put inside
/// itself, stands at the top, its `parent` cleared, so none is ever unreachable.
fn folder_order(folders: &[Folder]) -> Vec<Folder> {
    let known = |id: FolderId| folders.iter().any(|folder| folder.id == id);
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    let top = folders
        .iter()
        .filter(|folder| folder.parent.is_none_or(|parent| !known(parent)));
    for folder in top {
        walk(folders, folder, None, &mut seen, &mut out);
    }
    // What is left is a loop: each of it at the top.
    for folder in folders {
        walk(folders, folder, None, &mut seen, &mut out);
    }
    out
}

fn walk(
    folders: &[Folder],
    folder: &Folder,
    parent: Option<FolderId>,
    seen: &mut BTreeSet<FolderId>,
    out: &mut Vec<Folder>,
) {
    if !seen.insert(folder.id) {
        return;
    }
    out.push(Folder {
        parent,
        ..folder.clone()
    });
    for child in folders
        .iter()
        .filter(|child| child.parent == Some(folder.id))
    {
        walk(folders, child, Some(folder.id), seen, out);
    }
}

/// Every tab in sidebar order, the order ⌘1 to ⌘9 and next and previous use: the plan, read out.
pub fn order(state: &TabsState) -> Vec<TabId> {
    let plan = sidebar_plan(state);
    let mut all = plan.pinned;
    all.extend(plan.folders.into_iter().flat_map(|(_, tabs)| tabs));
    all.extend(plan.loose);
    all
}
