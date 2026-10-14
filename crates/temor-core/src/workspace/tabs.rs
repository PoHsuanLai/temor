//! The tab list: selecting, pinning, folders, unread marks and removal. Tabs stay until they
//! are removed (D3); the step never creates panes, that is the workspace step's job.

use std::collections::BTreeSet;

use super::sidebar::order;
use crate::space::{Folder, Pin, Place, Tab};
use crate::vocab::ids::{FolderId, PaneId, TabId};
use crate::vocab::text::{Relaunch, TabTitle};

/// The tab list as a Space keeps it, plus what the sidebar shows beside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabsState {
    /// The tabs, folders and the tab in front.
    pub place: Place,
    /// Tabs that got news while they were not in front.
    pub unread: BTreeSet<TabId>,
    /// The id the next folder takes.
    pub next_folder: u64,
}

impl Default for TabsState {
    fn default() -> Self {
        TabsState::from_place(Place::default())
    }
}

impl TabsState {
    /// A state over `place`, with the next folder id after the ones it holds.
    pub fn from_place(place: Place) -> TabsState {
        let next_folder = place
            .folders
            .iter()
            // A folder id at the top of the range leaves no id after it: the next `NewFolder` is
            // refused rather than reusing one.
            .map(|folder| folder.id.0.saturating_add(1))
            .max()
            .unwrap_or(1);
        TabsState {
            place,
            unread: BTreeSet::new(),
            next_folder,
        }
    }

    /// The tab with `id`, if it is in the list.
    pub fn tab(&self, id: TabId) -> Option<&Tab> {
        self.place.tabs.iter().find(|tab| tab.id == id)
    }

    /// Every pane of every tab, for the workspace's bookkeeping.
    pub fn panes(&self) -> Vec<PaneId> {
        self.place
            .tabs
            .iter()
            .flat_map(|tab| tab.tree.panes())
            .collect()
    }
}

/// A way to name a tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabSel {
    /// By identity.
    Id(TabId),
    /// By its place in the sidebar, 1 to 9 (⌘1 to ⌘9).
    Nth(NthTab),
    /// The next tab in the sidebar, wrapping from the last.
    Next,
    /// The previous tab in the sidebar, wrapping from the first.
    Previous,
}

/// A sidebar position from 1 to 9. Built only through [`NthTab::new`], so an out-of-range key
/// cannot make one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NthTab(u8);

impl NthTab {
    /// The position `number`, when it is 1 to 9.
    pub fn new(number: u8) -> Option<NthTab> {
        (1..=9).contains(&number).then_some(NthTab(number))
    }

    /// The position as a number from 1.
    pub fn number(self) -> u8 {
        self.0
    }
}

/// What the tab list is asked to do.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TabsIn {
    /// Bring a tab to the front (clears its unread mark).
    Select(TabSel),
    /// Pin a tab so it relaunches `relaunch` when temor starts.
    Pin { tab: TabId, relaunch: Relaunch },
    /// Make a pinned tab an ordinary one.
    Unpin(TabId),
    /// Group a tab in a folder, or out of every folder.
    MoveToFolder {
        tab: TabId,
        folder: Option<FolderId>,
    },
    /// Make a folder at the end of the sidebar's folders.
    NewFolder { title: TabTitle },
    /// Rename a tab.
    Rename { tab: TabId, title: TabTitle },
    /// Move a tab to where `at` stands in the list (a drag in the sidebar): after it when the
    /// tab came from above, before it when from below. Its pin and folder are its own.
    Place { tab: TabId, at: TabId },
    /// The tab got news (a command ended in one of its panes). Ignored in front.
    Unread(TabId),
    /// The tab closed: it leaves the list, and the focus moves to a neighbour.
    Remove(TabId),
}

/// What the tab list tells the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TabsOut {
    /// This tab is in front now.
    Focused(TabId),
    /// A folder was made.
    Created(FolderId),
    /// A tab left the list; the caller keeps it for the recently closed.
    Removed(Tab),
    /// The request was not carried out.
    Refused(TabRefusal),
}

/// Why a tab request was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TabRefusal {
    /// The tab is not in the list.
    NoSuchTab(TabId),
    /// The folder is not in the list.
    NoSuchFolder(FolderId),
    /// There is no tab at that sidebar position.
    NoTabAtSlot(NthTab),
    /// There are no tabs to move between.
    NoTabs,
    /// The folder counter has no id left to hand out.
    IdsExhausted,
}

/// The tab list after `input`.
pub fn tabs_step(mut state: TabsState, input: TabsIn) -> (TabsState, Vec<TabsOut>) {
    let out = match input {
        TabsIn::Select(sel) => select(&mut state, sel),
        TabsIn::Pin { tab, relaunch } => edit_tab(&mut state, tab, |t| {
            t.pin = Pin::Pinned { relaunch };
        }),
        TabsIn::Unpin(tab) => edit_tab(&mut state, tab, |t| t.pin = Pin::Loose),
        TabsIn::MoveToFolder { tab, folder } => move_to_folder(&mut state, tab, folder),
        TabsIn::NewFolder { title } => {
            let Some(next) = state.next_folder.checked_add(1) else {
                return (state, refuse(TabRefusal::IdsExhausted));
            };
            let id = FolderId(state.next_folder);
            state.next_folder = next;
            state.place.folders.push(Folder { id, title });
            vec![TabsOut::Created(id)]
        }
        TabsIn::Rename { tab, title } => edit_tab(&mut state, tab, |t| t.title = title),
        TabsIn::Place { tab, at } => place(&mut state, tab, at),
        TabsIn::Unread(tab) => unread(&mut state, tab),
        TabsIn::Remove(tab) => remove(&mut state, tab),
    };
    (state, out)
}

fn refuse(why: TabRefusal) -> Vec<TabsOut> {
    vec![TabsOut::Refused(why)]
}

fn select(state: &mut TabsState, sel: TabSel) -> Vec<TabsOut> {
    let ordered = order(state);
    let target = match sel {
        TabSel::Id(id) => {
            if state.tab(id).is_none() {
                return refuse(TabRefusal::NoSuchTab(id));
            }
            id
        }
        TabSel::Nth(nth) => match ordered.get(usize::from(nth.number()) - 1) {
            Some(id) => *id,
            None => return refuse(TabRefusal::NoTabAtSlot(nth)),
        },
        TabSel::Next | TabSel::Previous => {
            let Some(current) = state
                .place
                .focused
                .and_then(|f| ordered.iter().position(|t| *t == f))
            else {
                return match ordered.first() {
                    Some(first) => focus(state, *first),
                    None => refuse(TabRefusal::NoTabs),
                };
            };
            let last = ordered.len() - 1;
            let at = match sel {
                TabSel::Next if current == last => 0,
                TabSel::Next => current + 1,
                _ if current == 0 => last,
                _ => current - 1,
            };
            ordered[at]
        }
    };
    focus(state, target)
}

fn focus(state: &mut TabsState, id: TabId) -> Vec<TabsOut> {
    state.place.focused = Some(id);
    state.unread.remove(&id);
    vec![TabsOut::Focused(id)]
}

fn edit_tab(state: &mut TabsState, id: TabId, edit: impl FnOnce(&mut Tab)) -> Vec<TabsOut> {
    match state.place.tabs.iter_mut().find(|tab| tab.id == id) {
        Some(tab) => {
            edit(tab);
            Vec::new()
        }
        None => refuse(TabRefusal::NoSuchTab(id)),
    }
}

fn move_to_folder(state: &mut TabsState, id: TabId, folder: Option<FolderId>) -> Vec<TabsOut> {
    if let Some(wanted) =
        folder.filter(|wanted| !state.place.folders.iter().any(|f| f.id == *wanted))
    {
        return refuse(TabRefusal::NoSuchFolder(wanted));
    }
    edit_tab(state, id, |t| t.folder = folder)
}

fn place(state: &mut TabsState, id: TabId, at: TabId) -> Vec<TabsOut> {
    let tabs = &mut state.place.tabs;
    let Some(from) = tabs.iter().position(|tab| tab.id == id) else {
        return refuse(TabRefusal::NoSuchTab(id));
    };
    let Some(to) = tabs.iter().position(|tab| tab.id == at) else {
        return refuse(TabRefusal::NoSuchTab(at));
    };
    // Taken out and put back at `at`'s index: below it when it came from above, as a list
    // dragged by hand does.
    let moved = tabs.remove(from);
    tabs.insert(to, moved);
    Vec::new()
}

fn unread(state: &mut TabsState, id: TabId) -> Vec<TabsOut> {
    if state.tab(id).is_none() {
        return refuse(TabRefusal::NoSuchTab(id));
    }
    if state.place.focused != Some(id) {
        state.unread.insert(id);
    }
    Vec::new()
}

fn remove(state: &mut TabsState, id: TabId) -> Vec<TabsOut> {
    if state.tab(id).is_none() {
        return refuse(TabRefusal::NoSuchTab(id));
    }
    // The sidebar's order before the removal: the neighbour that takes the focus is the next
    // tab after it there, else the one before.
    let before = order(state);
    let at = before.iter().position(|t| *t == id).unwrap_or(0);
    let neighbour = before
        .get(at + 1)
        .or_else(|| at.checked_sub(1).and_then(|p| before.get(p)));
    let pos = state
        .place
        .tabs
        .iter()
        .position(|tab| tab.id == id)
        .unwrap_or(0);
    let removed = state.place.tabs.remove(pos);
    state.unread.remove(&id);
    let mut out = vec![TabsOut::Removed(removed)];
    if state.place.focused == Some(id) {
        state.place.focused = neighbour.copied();
        if let Some(next) = neighbour {
            out.push(TabsOut::Focused(*next));
        }
    }
    out
}
