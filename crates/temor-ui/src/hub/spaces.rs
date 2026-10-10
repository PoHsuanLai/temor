//! A workspace for every Space (PLAN §7). The Space on screen keeps its workspace in
//! `Hub::ws`, so every method that means "the current workspace" still does; the others are
//! parked here, with their panes still running (the pane slots are one map for all Spaces, and
//! pane ids are unique across them). Switching exchanges the two.

use std::collections::BTreeMap;

use ds::components::app::spaces::{SpaceId, Spaces};
use temor_core::notify::Focus;
use temor_core::space::{Place, Project, TemorSpaces};
use temor_core::workspace::{ClosedState, KeepClosed, Workspace, WorkspaceIn};

use super::model::{Hub, HubRef};
use crate::edge::{Credentials, space_agent::SpaceAgents};

/// The Spaces and the workspaces they keep.
pub(super) struct SpaceBook {
    /// The Spaces as the kit holds them (the window's copy is the kit's signal; this follows it
    /// whenever a change is final).
    pub(super) spaces: TemorSpaces,
    /// The Space whose workspace is `Hub::ws`.
    pub(super) active: SpaceId,
    /// The workspaces of the other Spaces.
    pub(super) parked: BTreeMap<SpaceId, Workspace>,
    /// Every pane id below this is taken by some Space.
    pub(super) floor: u64,
    pub(super) agents: SpaceAgents,
    pub(super) credentials: Credentials,
}

pub(super) fn empty_workspace() -> Workspace {
    Workspace::new(ClosedState::new(KeepClosed::DEFAULT))
}

impl SpaceBook {
    pub(super) fn new() -> SpaceBook {
        let spaces = Spaces::first_run([Project::default()], Project::default);
        SpaceBook {
            active: spaces.current().id,
            spaces,
            parked: BTreeMap::new(),
            floor: 1,
            agents: SpaceAgents::default(),
            credentials: Credentials::default(),
        }
    }

    /// The project of `id`; a Space the list does not know yet (just made) has the default one.
    pub(super) fn project_of(&self, id: SpaceId) -> Project {
        self.spaces
            .get(id)
            .map(|space| space.payload.clone())
            .unwrap_or_default()
    }

    /// Puts `current`, the active Space's workspace, away and takes the workspace of `to`
    /// (an empty one when it has none yet); `to` is active after.
    pub(super) fn exchange(&mut self, current: Workspace, to: SpaceId) -> Workspace {
        let next = self.parked.remove(&to).unwrap_or_else(empty_workspace);
        self.parked.insert(self.active, current);
        self.active = to;
        next
    }

    /// `ws` with its pane counter past every pane any Space has.
    pub(super) fn lift(&self, ws: &mut Workspace) {
        ws.next_pane = ws.next_pane.max(self.floor);
    }

    /// Takes in the ids `ws` has handed out.
    pub(super) fn note(&mut self, ws: &Workspace) {
        self.floor = self.floor.max(ws.next_pane);
    }

    /// The Space that has `pane` in a parked workspace.
    pub(super) fn parked_owner(&self, pane: temor_core::vocab::ids::PaneId) -> Option<SpaceId> {
        self.parked
            .iter()
            .find(|(_, ws)| ws.tab_of(pane).is_some())
            .map(|(id, _)| *id)
    }
}

/// What the Spaces file keeps of a workspace: its place and the files its viewer panes show.
pub(super) fn recall_of(ws: &Workspace) -> Place {
    ws.tabs
        .place
        .clone()
        .with_viewers(&ws.viewers)
        .with_cwds(&ws.cwds)
}

impl Hub {
    /// The Space on screen.
    pub fn active_space(&self) -> SpaceId {
        self.book.active
    }

    /// The Spaces, as the window last made them final.
    pub fn spaces(&self) -> &TemorSpaces {
        &self.book.spaces
    }

    /// Gives the window the account store to fill Spaces' credentials from.
    pub fn set_credentials(&mut self, credentials: Credentials) {
        self.book.credentials = credentials;
    }

    /// Where the Space on screen is, for the kit to remember when it is left.
    pub fn here(&self) -> Place {
        self.ws.tabs.place.clone()
    }

    /// Shows the workspace of `to`: the Space on screen is put away, with its panes running.
    pub(super) fn swap_to(&mut self, to: SpaceId) {
        if to == self.book.active {
            return;
        }
        let current = std::mem::replace(&mut self.ws, empty_workspace());
        self.ws = self.book.exchange(current, to);
    }

    /// Runs `work` with the workspace of `space` as the current one, then puts things back.
    pub(super) fn with_space<T>(&mut self, space: SpaceId, work: impl FnOnce(&mut Hub) -> T) -> T {
        let back = self.book.active;
        self.swap_to(space);
        let out = work(self);
        self.swap_to(back);
        out
    }

    /// The kit switched to `to`: its workspace comes to the front, with a first tab if it has
    /// none, and the keyboard follows.
    pub fn arrive(&mut self, to: SpaceId) {
        if to == self.book.active {
            return;
        }
        if let Some(slot) = self.keyboard_pane().and_then(|pane| self.slots.get(&pane))
            && let Some(session) = slot.borrow_mut().session.as_mut()
        {
            session.set_focus(Focus::Unfocused);
        }
        self.swap_to(to);
        self.fill_empty();
        self.refresh_probe();
        self.pace.changed();
    }

    /// The parked Space that holds the pane `input` is about, when the Space on screen does not.
    pub(super) fn parked_owner_of(&self, input: &WorkspaceIn) -> Option<SpaceId> {
        let pane = match input {
            WorkspaceIn::Activity(pane)
            | WorkspaceIn::Focus(pane)
            | WorkspaceIn::Tear(pane)
            | WorkspaceIn::ClosePane(Some(pane))
            | WorkspaceIn::Cwd { pane, .. } => *pane,
            _ => return None,
        };
        match self.ws.tab_of(pane) {
            Some(_) => None,
            None => self.book.parked_owner(pane),
        }
    }

    /// A Space with no tab gets one, as a window closing its last tab does.
    pub(super) fn fill_empty(&mut self) {
        if self.ws.tabs.place.tabs.is_empty() {
            self.act(WorkspaceIn::NewTab { cwd: None });
        }
    }

    /// The Spaces changed for good (a switch, a new or renamed or recoloured or deleted one):
    /// follow them, close what a deleted Space had, and note the file wants writing.
    pub fn spaces_changed(&mut self, all: &TemorSpaces) {
        self.book.spaces = all.clone();
        let gone: Vec<SpaceId> = self
            .book
            .parked
            .keys()
            .filter(|id| all.get(**id).is_none())
            .copied()
            .collect();
        for id in gone {
            self.forget_space(id);
        }
        self.pace.changed();
    }

    /// The Spaces with each one's place brought up to date, as the file keeps them.
    pub(super) fn spaces_for_file(&self) -> TemorSpaces {
        let mut all = self.book.spaces.clone();
        let places = self
            .book
            .parked
            .iter()
            .map(|(id, ws)| (*id, recall_of(ws)))
            .chain([(self.book.active, recall_of(&self.ws))]);
        for (id, place) in places {
            all.edit_recall(id, |recalled| *recalled = place);
        }
        all
    }
}

impl HubRef {
    /// The kit switched Space: bring its workspace to the front. Returns the notices left.
    pub fn arrive(&self, to: SpaceId) -> Vec<String> {
        let mut hub = self.0.borrow_mut();
        hub.arrive(to);
        std::mem::take(&mut hub.notices)
    }

    /// The Spaces changed for good. Returns the notices left.
    pub fn spaces_changed(&self, all: &TemorSpaces) -> Vec<String> {
        let mut hub = self.0.borrow_mut();
        hub.spaces_changed(all);
        std::mem::take(&mut hub.notices)
    }

    /// The Spaces to start the kit with.
    pub fn boot_spaces(&self) -> TemorSpaces {
        self.0.borrow().book.spaces.clone()
    }

    /// Where the Space on screen is.
    pub fn here(&self) -> Place {
        self.0.borrow().here()
    }

    /// The Space on screen.
    pub fn active_space(&self) -> SpaceId {
        self.0.borrow().active_space()
    }
}
