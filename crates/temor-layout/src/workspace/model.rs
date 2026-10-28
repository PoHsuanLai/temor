//! The workspace's state: the tab list, each tab's focused pane, every pane's working folder,
//! the recently closed tabs, and the next ids to hand out.

use std::collections::BTreeMap;

use super::closed::ClosedState;
use super::tabs::{TabSel, TabsIn, TabsOut, TabsState, tabs_step};
use crate::space::{Pin, Tab};
use crate::split::SplitTree;
use temor_core::vocab::ids::{PaneId, TabId};
use temor_core::vocab::text::{Cwd, TabTitle, ViewerRef};

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

    /// Runs one step on the tab list and keeps the list it gives back; what the step said.
    pub(crate) fn tabs_do(&mut self, input: TabsIn) -> Vec<TabsOut> {
        let (tabs, outs) = tabs_step(std::mem::take(&mut self.tabs), input);
        self.tabs = tabs;
        outs
    }

    /// Brings `tab` to the front.
    pub(crate) fn bring_forward(&mut self, tab: TabId) {
        self.tabs_do(TabsIn::Select(TabSel::Id(tab)));
    }

    /// Puts `tab` at the end of the list, with `focus` the pane that has keys in it, and brings
    /// it to the front. The caller has minted the ids and tells the window.
    pub(crate) fn open_tab(&mut self, tab: Tab, focus: PaneId) {
        let id = tab.id;
        self.tabs.place.tabs.push(tab);
        self.focus.insert(id, focus);
        self.bring_forward(id);
    }

    /// A new loose tab `id` named "Shell" around `pane`, in front, with the folder the pane
    /// starts in when it has one.
    pub(crate) fn open_shell_tab(&mut self, id: TabId, pane: PaneId, cwd: Option<Cwd>) {
        self.open_tab(
            Tab {
                id,
                title: TabTitle("Shell".to_owned()),
                pin: Pin::Loose,
                folder: None,
                tree: SplitTree::single(pane),
            },
            pane,
        );
        if let Some(cwd) = cwd {
            self.cwds.insert(pane, cwd);
        }
    }

    /// Forgets the folder and the file recorded for `pane`.
    pub(crate) fn forget_pane(&mut self, pane: PaneId) {
        self.cwds.remove(&pane);
        self.viewers.remove(&pane);
    }
}
