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
    /// Unpinned tabs without a folder, in list order.
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
    let loose = state
        .place
        .tabs
        .iter()
        .filter(|tab| !is_pinned(tab) && tab.folder.is_none())
        .map(|tab| tab.id)
        .collect();
    SidebarPlan {
        pinned,
        folders,
        loose,
    }
}

/// Every tab in sidebar order, the order ⌘1 to ⌘9 and next and previous use. A tab in a folder
/// that no longer exists is listed with the loose tabs, so no tab is ever unreachable.
pub fn order(state: &TabsState) -> Vec<TabId> {
    let plan = sidebar_plan(state);
    let mut all = plan.pinned;
    all.extend(plan.folders.into_iter().flat_map(|(_, tabs)| tabs));
    all.extend(plan.loose);
    // Tabs in a folder that is gone fall through the plan; add them after the rest.
    for tab in &state.place.tabs {
        if !all.contains(&tab.id) {
            all.push(tab.id);
        }
    }
    all
}
