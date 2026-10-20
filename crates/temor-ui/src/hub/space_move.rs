//! Moving things between Spaces: a tab to another Space, a running pane adopted as a tab (the
//! quick terminal's "Move to Space"), and what a deleted Space leaves behind.

use ds::components::app::spaces::SpaceId;
use temor_core::error::CoreError;
use temor_core::quick::adopt;
use temor_core::space::carry_tab;
use temor_core::vocab::ids::{PaneId, TabId};
use temor_core::vocab::text::Cwd;
use temor_core::workspace::WorkspaceOut;

use super::model::{Hub, HubRef};
use crate::edge::remove_session_for;

impl Hub {
    /// Moves `tab` of the Space on screen to the Space `to`. Its panes keep running; the tab is
    /// loose there, behind the tabs `to` has. Moving to the Space it is in does nothing.
    pub fn move_tab_to_space(&mut self, tab: TabId, to: SpaceId) {
        if to == self.book.active {
            return;
        }
        if self.book.spaces.get(to).is_none() {
            self.notices.push(CoreError::UnknownSpace(to.0).to_string());
            return;
        }
        let there = self.book.parked_or_empty(to);
        match carry_tab(&self.ws, tab, &there) {
            Ok((here, there)) => {
                self.book.note(&there);
                self.ws = here;
                self.book.parked.insert(to, there);
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
        if self.book.spaces.get(space).is_none() {
            return Err(CoreError::UnknownSpace(space.0));
        }
        let current = space == self.book.active;
        let base = match current {
            true => self.ws.clone(),
            false => self.book.parked_or_empty(space),
        };
        if self.owner_of(pane).is_some() {
            return Err(CoreError::DuplicatePane(pane.0));
        }
        let (ws, outs) = adopt(&base, pane, cwd)?;
        self.book.note(&ws);
        match current {
            true => self.ws = ws,
            false => {
                self.book.parked.insert(space, ws);
            }
        }
        for out in outs {
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
        match self.ws.tab_of(pane).is_some() {
            true => Some(self.book.active),
            false => self.book.parked_owner(pane),
        }
    }

    /// A deleted Space: its panes end, its stored scrollback and session file go, and so does
    /// its key agent.
    pub(super) fn forget_space(&mut self, id: SpaceId) {
        if let Some(ws) = self.book.parked.remove(&id) {
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
        self.book.agents.stop(id);
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
