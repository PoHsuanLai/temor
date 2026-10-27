//! Moving things between Spaces: a tab to another Space, a running pane adopted as a tab (the
//! quick terminal's "Move to Space"), and what a deleted Space leaves behind.

use ds::components::app::spaces::SpaceId;
use temor_core::error::CoreError;
use temor_core::vocab::ids::{PaneId, TabId};
use temor_core::vocab::text::Cwd;
use temor_core::workspace::WorkspaceOut;

use super::model::{Hub, HubRef};
use crate::edge::remove_session_for;

impl Hub {
    /// Moves `tab` of the Space on screen to the Space `to`. Its panes keep running; the tab is
    /// loose there, behind the tabs `to` has. Moving to the Space it is in does nothing.
    pub fn move_tab_to_space(&mut self, tab: TabId, to: SpaceId) {
        match self.book.carry(tab, to) {
            Ok(false) => {}
            Ok(true) => {
                // The Space it left may have been left with nothing.
                self.fill_empty();
                self.refresh_probe();
                self.pace.changed();
            }
            Err(err) => self.notices.push(err.to_string()),
        }
    }

    /// Gives the running pane `pane` (already in the hub's slots) to the Space `space` as a tab
    /// of its own, in front there, started in `cwd`. For the quick terminal's "Move to Space";
    /// the Space shown does not change. Refused when the Space is gone or the pane is a tab's.
    pub fn adopt_into_space(
        &mut self,
        space: SpaceId,
        pane: PaneId,
        cwd: Option<Cwd>,
    ) -> Result<(), CoreError> {
        let current = space == self.book.active();
        for out in self.book.adopt(space, pane, cwd)? {
            match out {
                WorkspaceOut::Focused(_) if current => self.refresh_probe(),
                WorkspaceOut::Changed => self.pace.changed(),
                _ => {}
            }
        }
        Ok(())
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
        let mut hub = self.0.borrow_mut();
        hub.move_tab_to_space(tab, to);
        std::mem::take(&mut hub.notices)
    }

    /// See [`Hub::adopt_into_space`].
    pub fn adopt_into_space(
        &self,
        space: SpaceId,
        pane: PaneId,
        cwd: Option<Cwd>,
    ) -> Result<(), CoreError> {
        self.0.borrow_mut().adopt_into_space(space, pane, cwd)
    }
}
