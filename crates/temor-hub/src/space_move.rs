//! Moving things between Spaces: a tab to another Space, a running pane adopted as a tab (the
//! quick terminal's "Move to Space"), and what a deleted Space leaves behind.

use ds::components::app::spaces::SpaceId;
use temor_client::model::error::CoreError;
use temor_client::model::vocab::ids::{PaneId, TabId};
use temor_client::model::vocab::text::Cwd;

use super::desk::Desk;
use super::model::{Hub, HubRef};
use temor_client::keep::older::remove_for as remove_session_for;

impl Hub {
    /// Moves `tab` of the Space on screen to the Space `to`. Its panes keep running; the tab is
    /// loose there, behind the tabs `to` has. Moving to the Space it is in does nothing.
    pub fn move_tab_to_space(&mut self, tab: TabId, to: SpaceId) {
        match self.book.carry(tab, to) {
            Ok(false) => {}
            Ok(true) => {
                // The Space it left may have been left with nothing.
                self.fill_empty();
            }
            Err(err) => self.notices.push(err.to_string()),
        }
    }

    /// Gives the running pane `pane` (already in the hub's slots) to the Space `space` as a tab
    /// of its own, in front there, started in `cwd`. For the quick terminal's "Move to Space";
    /// the Space shown does not change. Refused when the Space is gone or the pane is a tab's.
    pub(super) fn adopt_into_space(
        &mut self,
        space: SpaceId,
        pane: PaneId,
        cwd: Option<Cwd>,
    ) -> Result<(), CoreError> {
        // What the step answers is the focus, which `Hub::sync_focus` follows.
        self.book.adopt(space, pane, cwd).map(drop)
    }

    /// The Space whose tabs hold `pane`.
    pub fn owner_of(&self, pane: PaneId) -> Option<SpaceId> {
        self.book.owner_of(pane)
    }

    /// A deleted Space: its panes end, its stored scrollback and session file go, and so does
    /// its key agent.
    pub(super) fn forget_space(&mut self, id: SpaceId) {
        if let Some(ws) = self.book.unpark(id) {
            let closed = ws
                .closed
                .tabs
                .iter()
                .flat_map(|closed| closed.tab.tree.panes());
            for pane in ws.tabs.panes().into_iter().chain(closed) {
                self.drop_pane(pane);
                self.forget_pane(pane);
            }
        }
        if let Some(dir) = &self.state_dir {
            remove_session_for(dir, id);
        }
        self.keep.agents.stop(id);
    }
}

impl HubRef {
    /// Moves `tab` of the Space on screen to the Space `to`; returns the notices left.
    pub fn move_tab_to_space(&self, tab: TabId, to: SpaceId) -> Vec<String> {
        self.said(|hub| hub.move_tab_to_space(tab, to))
    }
}

impl Desk {
    /// Moves the tab in front to `space`.
    pub fn move_front_tab_to(&self, space: SpaceId) {
        if let Some((tab, _, _)) = self.hub.front() {
            self.tell(self.hub.move_tab_to_space(tab, space));
        }
    }
}
