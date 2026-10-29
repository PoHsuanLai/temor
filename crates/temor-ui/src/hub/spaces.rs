//! The Spaces on the window's side (PLAN §7). Their workspaces are the client's
//! ([`temor_client::spaces::Book`]); the window carries out what a switch leaves to do (the
//! keyboard follows, an empty Space gets a tab) and keeps each Space's key agent and the account
//! store its credentials come from.

use ds::components::app::spaces::SpaceId;
use temor_client::model::notify::Focus;
use temor_client::model::space::{Place, TemorSpaces};
use temor_client::model::workspace::WorkspaceIn;

use super::model::{Hub, HubRef};
use crate::edge::Credentials;
use crate::settings_window::SpaceRow;
use temor_client::space::agent::SpaceAgents;

/// What the window keeps of each Space besides its workspace.
#[derive(Default)]
pub(super) struct SpaceKeep {
    pub(super) agents: SpaceAgents,
    pub(super) credentials: Credentials,
}

impl Hub {
    /// The Space on screen.
    pub fn active_space(&self) -> SpaceId {
        self.book.active()
    }

    /// The Spaces, as the window last made them final.
    pub fn spaces(&self) -> &TemorSpaces {
        self.book.spaces()
    }

    /// Gives the window the account store to fill Spaces' credentials from.
    pub fn set_credentials(&mut self, credentials: Credentials) {
        self.keep.credentials = credentials;
    }

    /// Where the Space on screen is, for the kit to remember when it is left.
    pub fn here(&self) -> Place {
        self.book.here()
    }

    /// Shows the workspace of `to`: the Space on screen is put away, with its panes running.
    pub(super) fn swap_to(&mut self, to: SpaceId) {
        self.book.swap_to(to);
    }

    /// Runs `work` with the workspace of `space` as the current one, then puts things back.
    pub(super) fn with_space<T>(&mut self, space: SpaceId, work: impl FnOnce(&mut Hub) -> T) -> T {
        let back = self.book.active();
        self.swap_to(space);
        let out = work(self);
        self.swap_to(back);
        out
    }

    /// The kit switched to `to`: its workspace comes to the front, with a first tab if it has
    /// none, and the keyboard follows.
    pub fn arrive(&mut self, to: SpaceId) {
        if to == self.book.active() {
            return;
        }
        if let Some(pane) = self.keyboard_pane() {
            self.set_pane_focus(pane, Focus::Unfocused);
        }
        self.swap_to(to);
        self.fill_empty();
        self.refresh_probe();
        self.pace.changed();
    }

    /// A Space with no tab gets one, as a window closing its last tab does.
    pub(super) fn fill_empty(&mut self) {
        if self.book.ws.tabs.place.tabs.is_empty() {
            self.act(WorkspaceIn::NewTab { cwd: None });
        }
    }

    /// The Spaces changed for good (a switch, a new or renamed or recoloured or deleted one):
    /// follow them, close what a deleted Space had, and note the file wants writing.
    pub fn spaces_changed(&mut self, all: &TemorSpaces) {
        for id in self.book.follow(all) {
            self.forget_space(id);
        }
        self.pace.changed();
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

    /// The Spaces as the Settings window shows them: each one's name and project.
    pub fn space_rows(&self) -> Vec<SpaceRow> {
        self.0
            .borrow()
            .book
            .spaces()
            .list()
            .iter()
            .map(|space| SpaceRow {
                id: space.id,
                name: space.name.clone(),
                project: space.payload.clone(),
            })
            .collect()
    }

    /// The Spaces to start the kit with.
    pub fn boot_spaces(&self) -> TemorSpaces {
        self.0.borrow().book.spaces().clone()
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
