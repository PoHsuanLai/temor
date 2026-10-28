//! A workspace for every Space (PLAN §7, §2b P24c). The Space on screen keeps its workspace in
//! [`Book::ws`]; the others are parked here with their panes still running (pane ids are unique
//! across Spaces, so one pane table serves them all). Every request is answered at once, on the
//! caller's thread: a window acts on the answer before it draws again.
//!
//! The book keeps tabs and Spaces, not panes: what a step answers ([`WorkspaceOut`]) is the
//! window's to carry out.

use std::collections::BTreeMap;

use ds_style::space::list::{SpaceId, Spaces};
use temor_core::error::CoreError;
use temor_core::vocab::ids::{PaneId, TabId};
use temor_core::vocab::text::Cwd;
use temor_layout::quick::adopt;
use temor_layout::space::{Place, Project, TemorSpaces, carry_tab};
use temor_layout::workspace::restore::{Restored, VERSION, decode};
use temor_layout::workspace::{
    ClosedState, KeepClosed, Workspace, WorkspaceIn, WorkspaceOut, workspace_step,
};

/// A workspace with no tab.
pub fn empty_workspace() -> Workspace {
    Workspace::new(ClosedState::new(KeepClosed::DEFAULT))
}

/// What the Spaces keep of a workspace: its place, the files its viewer panes show and its
/// panes' folders.
pub fn recall_of(ws: &Workspace) -> Place {
    ws.tabs
        .place
        .clone()
        .with_viewers(&ws.viewers)
        .with_cwds(&ws.cwds)
}

/// The Spaces and the workspaces they keep.
#[derive(Debug)]
pub struct Book {
    /// The workspace of the Space on screen.
    pub ws: Workspace,
    /// The Spaces as the kit last made them final.
    spaces: TemorSpaces,
    /// The Space whose workspace is `ws`.
    active: SpaceId,
    /// The workspaces of the other Spaces.
    parked: BTreeMap<SpaceId, Workspace>,
    /// Every pane id below this is taken by some Space.
    floor: u64,
}

impl Default for Book {
    fn default() -> Self {
        Book::new()
    }
}

impl Book {
    /// One Space, with no tab.
    pub fn new() -> Book {
        let spaces = Spaces::first_run([Project::default()], Project::default);
        Book {
            ws: empty_workspace(),
            active: spaces.current().id,
            spaces,
            parked: BTreeMap::new(),
            floor: 1,
        }
    }

    /// The Space on screen.
    pub fn active(&self) -> SpaceId {
        self.active
    }

    /// The Spaces.
    pub fn spaces(&self) -> &TemorSpaces {
        &self.spaces
    }

    /// Starts over with `spaces`, the one they call current on screen. The workspaces stay.
    pub fn begin(&mut self, spaces: TemorSpaces) {
        self.active = spaces.current().id;
        self.spaces = spaces;
    }

    /// The project of `id`; a Space the list does not know yet (just made) has the default one.
    pub fn project_of(&self, id: SpaceId) -> Project {
        self.spaces
            .get(id)
            .map(|space| space.payload.clone())
            .unwrap_or_default()
    }

    /// Where the Space on screen is.
    pub fn here(&self) -> Place {
        self.ws.tabs.place.clone()
    }

    /// Runs `input` through the workspace of the Space on screen and answers what it says to
    /// do. Pane ids it hands out are past every pane any Space has.
    pub fn step(&mut self, input: WorkspaceIn) -> Vec<WorkspaceOut> {
        let mut ws = std::mem::replace(&mut self.ws, empty_workspace());
        ws.next_pane = ws.next_pane.max(self.floor);
        let (ws, outs) = workspace_step(ws, input);
        self.ws = ws;
        self.note_ws();
        outs
    }

    /// Takes in the ids the workspace on screen has handed out.
    fn note_ws(&mut self) {
        self.floor = self.floor.max(self.ws.next_pane);
    }

    /// Takes in the ids `ws` has handed out.
    fn note(&mut self, ws: &Workspace) {
        self.floor = self.floor.max(ws.next_pane);
    }

    /// Shows the workspace of `to` (an empty one when it has none yet); the Space on screen is
    /// put away, its panes running.
    pub fn swap_to(&mut self, to: SpaceId) {
        if to == self.active {
            return;
        }
        let next = self.parked.remove(&to).unwrap_or_else(empty_workspace);
        let current = std::mem::replace(&mut self.ws, next);
        self.parked.insert(self.active, current);
        self.active = to;
    }

    /// Keeps `ws` as the workspace of `id`, on screen when `id` is the Space on screen.
    pub fn place(&mut self, id: SpaceId, ws: Workspace) {
        self.note(&ws);
        match id == self.active {
            true => self.ws = ws,
            false => {
                self.parked.insert(id, ws);
            }
        }
    }

    /// The parked workspaces.
    pub fn parked(&self) -> impl Iterator<Item = (SpaceId, &Workspace)> {
        self.parked.iter().map(|(id, ws)| (*id, ws))
    }

    /// Every Space's workspace, the one on screen last.
    pub fn every(&self) -> impl Iterator<Item = (SpaceId, &Workspace)> {
        self.parked().chain([(self.active, &self.ws)])
    }

    /// The workspace the Spaces recall for `id`: its tabs and folders, each shell starting in
    /// the Space's folder. `None` when the place holds no tab.
    pub fn recalled(&self, id: SpaceId) -> Option<Workspace> {
        let place = self.spaces.recall().of(id);
        let text = serde_json::json!({ "version": VERSION, "place": place }).to_string();
        match decode(&text, KeepClosed::DEFAULT) {
            Restored::Loaded(ws) if !ws.tabs.place.tabs.is_empty() => Some(*ws),
            _ => None,
        }
    }

    /// The Space whose tabs hold `pane`.
    pub fn owner_of(&self, pane: PaneId) -> Option<SpaceId> {
        match self.ws.tab_of(pane).is_some() {
            true => Some(self.active),
            false => self.parked_owner(pane),
        }
    }

    /// The parked Space that holds `pane`.
    fn parked_owner(&self, pane: PaneId) -> Option<SpaceId> {
        self.parked
            .iter()
            .find(|(_, ws)| ws.tab_of(pane).is_some())
            .map(|(id, _)| *id)
    }

    /// The parked Space that holds the pane `input` is about, when the Space on screen does not.
    pub fn parked_owner_of(&self, input: &WorkspaceIn) -> Option<SpaceId> {
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
            None => self.parked_owner(pane),
        }
    }

    /// Moves `tab` of the Space on screen to the Space `to`, loose there behind its tabs. Its
    /// panes keep running. Answers whether anything moved: moving to the Space it is in does
    /// nothing.
    pub fn carry(&mut self, tab: TabId, to: SpaceId) -> Result<bool, CoreError> {
        if to == self.active {
            return Ok(false);
        }
        if self.spaces.get(to).is_none() {
            return Err(CoreError::UnknownSpace(to.0));
        }
        let there = self
            .parked
            .get(&to)
            .cloned()
            .unwrap_or_else(empty_workspace);
        let (here, there) = carry_tab(&self.ws, tab, &there)?;
        self.note(&there);
        self.ws = here;
        self.parked.insert(to, there);
        Ok(true)
    }

    /// Gives `pane`, running but in no tab, to the Space `space` as a tab of its own, in front
    /// there, started in `cwd`. Refused when the Space is gone or a tab already has the pane.
    pub fn adopt(
        &mut self,
        space: SpaceId,
        pane: PaneId,
        cwd: Option<Cwd>,
    ) -> Result<Vec<WorkspaceOut>, CoreError> {
        if self.spaces.get(space).is_none() {
            return Err(CoreError::UnknownSpace(space.0));
        }
        if self.owner_of(pane).is_some() {
            return Err(CoreError::DuplicatePane(pane.0));
        }
        let base = match space == self.active {
            true => self.ws.clone(),
            false => self
                .parked
                .get(&space)
                .cloned()
                .unwrap_or_else(empty_workspace),
        };
        let (ws, outs) = adopt(&base, pane, cwd)?;
        self.place(space, ws);
        Ok(outs)
    }

    /// The Spaces changed for good: follow them. Answers the Spaces that are gone and still have
    /// a workspace parked, which the caller [unparks](Book::unpark) and whose panes it ends.
    pub fn follow(&mut self, all: &TemorSpaces) -> Vec<SpaceId> {
        self.spaces = all.clone();
        self.parked
            .keys()
            .filter(|id| all.get(**id).is_none())
            .copied()
            .collect()
    }

    /// Takes the parked workspace of `id` away, as a Space is deleted.
    pub fn unpark(&mut self, id: SpaceId) -> Option<Workspace> {
        self.parked.remove(&id)
    }

    /// The Spaces with each one's place brought up to date, as they are kept.
    pub fn for_keeping(&self) -> TemorSpaces {
        let mut all = self.spaces.clone();
        for (id, ws) in self.every() {
            let place = recall_of(ws);
            all.edit_recall(id, |recalled| *recalled = place);
        }
        all
    }
}

#[cfg(test)]
#[path = "spaces_tests.rs"]
mod tests;
