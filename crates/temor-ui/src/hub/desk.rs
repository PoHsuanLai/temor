//! What a view holds to ask the window for things: the hub, the signal that redraws the window,
//! and the signal the message bar reads. Cloned into every pane view and the sidebar.

use dioxus::prelude::*;
use ds::base::vocab::Shown;
use ds::window::host::WindowHost;
use ds::window::vocab::PageZoom;
use temor_client::model::vocab::ids::PaneId;
use temor_client::model::workspace::WorkspaceIn;

use super::model::HubRef;
use super::space_rows::PendingSwitch;
use crate::view::space_handle::SpacesOf;

/// The window's desk. Two desks are equal when they are the same window's.
#[derive(Clone, PartialEq)]
pub struct Desk {
    pub hub: HubRef,
    /// Bumped to draw the window again; views read it to follow the hub.
    pub ui: Signal<u32>,
    /// The message the person sees above the panes.
    pub notice: Signal<Option<String>>,
    /// The Spaces: the kit's handle, which switches, edits and writes them.
    pub spaces: SpacesOf,
    /// The Space the ⌘T bar asked for, until the desk switches to it.
    pub picked_space: PendingSwitch,
    /// Whether the sidebar is shown: the window's, which the show/hide action flips.
    pub sidebar: Signal<Shown>,
    /// The window itself, which zooms the page; none where nothing hosts the window.
    pub window: HostOf,
}

/// The window's host, if there is one. Always equal: a desk holds its own window's.
#[derive(Debug, Clone, Default)]
pub struct HostOf(pub Option<WindowHost>);

impl PartialEq for HostOf {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl HostOf {
    /// Zooms the page one step, or back to its own size.
    pub fn page_zoom(&self, step: PageZoom) {
        if let Some(host) = &self.0 {
            host.page_zoom(step);
        }
    }
}

impl std::fmt::Debug for Desk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Desk").finish_non_exhaustive()
    }
}

impl Desk {
    /// Draws the window again.
    pub fn bump(&self) {
        let mut ui = self.ui;
        ui += 1;
    }

    /// Shows `text` to the person.
    pub fn say(&self, text: String) {
        let mut notice = self.notice;
        notice.set(Some(text));
    }

    /// Shows `notes` to the person, and draws the window again.
    pub fn tell(&self, notes: impl IntoIterator<Item = String>) {
        for text in notes {
            self.say(text);
        }
        self.bump();
    }

    /// Runs a workspace request, shows what it has to say, and draws the window again.
    pub fn act(&self, input: WorkspaceIn) {
        self.tell(self.hub.act(input));
    }

    /// Starts a new shell in a pane whose program ended.
    pub fn restart(&self, pane: PaneId) {
        self.tell(self.hub.restart(pane));
    }
}
