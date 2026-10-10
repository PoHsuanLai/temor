//! The workspace's state: the tab list, each tab's focused pane, every pane's working folder,
//! the recently closed tabs, and the next ids to hand out.

use std::collections::BTreeMap;

use super::closed::ClosedState;
use super::tabs::TabsState;
use crate::vocab::ids::{PaneId, TabId};
use crate::vocab::text::{Cwd, ViewerRef};

/// Everything the window shows that is not inside a pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    /// The tab list.
    pub tabs: TabsState,
    /// The pane that has keys in each tab.
    pub focus: BTreeMap<TabId, PaneId>,
    /// Each pane's working folder, as far as it is known (OSC 7). A closed tab keeps its panes'
    /// folders until they are forgotten, so a reopen can start in them.
    pub cwds: BTreeMap<PaneId, Cwd>,
    /// The panes that show a file instead of a program, and which file. A closed tab keeps its
    /// viewers until it is forgotten, so a reopen can show them again.
    pub viewers: BTreeMap<PaneId, ViewerRef>,
    /// The recently closed tabs.
    pub closed: ClosedState,
    /// The id the next tab takes.
    pub next_tab: u64,
    /// The id the next pane takes.
    pub next_pane: u64,
}

impl Workspace {
    /// An empty workspace: no tabs, ids from 1.
    pub fn new(closed: ClosedState) -> Workspace {
        Workspace {
            tabs: TabsState::default(),
            focus: BTreeMap::new(),
            cwds: BTreeMap::new(),
            viewers: BTreeMap::new(),
            closed,
            next_tab: 1,
            next_pane: 1,
        }
    }

    /// The tab that holds `pane`, if any.
    pub fn tab_of(&self, pane: PaneId) -> Option<TabId> {
        self.tabs
            .place
            .tabs
            .iter()
            .find(|tab| tab.tree.contains(pane))
            .map(|tab| tab.id)
    }

    /// The tab in front.
    pub fn front(&self) -> Option<TabId> {
        self.tabs.place.focused
    }

    /// The pane with keys in `tab`: the recorded one, else the tab's first pane.
    pub fn focus_of(&self, tab: TabId) -> Option<PaneId> {
        let t = self.tabs.tab(tab)?;
        self.focus
            .get(&tab)
            .copied()
            .filter(|pane| t.tree.contains(*pane))
            .or_else(|| t.tree.panes().first().copied())
    }
}
