//! The sidebar's order (PLAN §7): pinned tabs first, then each folder's tabs, then the rest.
//! ⌘1 to ⌘9 count along the same order, so a key and a row always name the same tab.

use super::tabs::TabsState;
use crate::space::{Folder, Pin};
use crate::vocab::ids::TabId;

/// Where each tab sits in the sidebar. A tab that is pinned appears under pinned, whatever its
/// folder; a folder with no tabs is still listed (the UI may leave it out).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SidebarPlan {
    /// Pinned tabs, in list order.
    pub pinned: Vec<TabId>,
    /// Each folder with its unpinned tabs, in folder order.
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
    let folders = state
        .place
        .folders
        .iter()
        .map(|folder| {
            let tabs = state
                .place
                .tabs
                .iter()
                .filter(|tab| !is_pinned(tab) && tab.folder == Some(folder.id))
                .map(|tab| tab.id)
                .collect();
            (folder.clone(), tabs)
        })
        .collect();
    let has_folder = |tab: &crate::space::Tab| {
        tab.folder
            .is_some_and(|id| state.place.folders.iter().any(|folder| folder.id == id))
    };
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

/// Every tab in sidebar order, the order ⌘1 to ⌘9 and next and previous use: the plan, read out.
pub fn order(state: &TabsState) -> Vec<TabId> {
    let plan = sidebar_plan(state);
    let mut all = plan.pinned;
    all.extend(plan.folders.into_iter().flat_map(|(_, tabs)| tabs));
    all.extend(plan.loose);
    all
}
