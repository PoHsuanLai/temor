//! The hub's side of the radar: what the panes' events and the window's focus tell it, the
//! waiting count the badge reads, and ⌘⇧U.

use std::path::PathBuf;

use temor_core::presence::AgentState;
use temor_core::vocab::ids::{PaneId, TabId};
use temor_core::workspace::{TabSel, TabsIn, WorkspaceIn};
use temor_pane::PaneEvent;

use super::radar::{Alert, Changed};
use crate::hub::model::Hub;

/// How many agent panes wait for the person. The desktop's badge reads this: a trait, so the
/// badge's code needs the count and nothing else of the hub.
pub trait WaitingCount {
    fn waiting_count(&self) -> usize;
}

impl WaitingCount for Hub {
    fn waiting_count(&self) -> usize {
        self.agents.radar.waiting().len()
    }
}

impl WaitingCount for crate::hub::model::HubRef {
    fn waiting_count(&self) -> usize {
        self.0.borrow().agents.radar.waiting().len()
    }
}

impl Hub {
    /// How many agent panes wait for the person.
    pub fn waiting_count(&self) -> usize {
        WaitingCount::waiting_count(self)
    }

    /// The notifications the radar wants made now: a pane that needs the person while it is not
    /// the one in use, with the agent's own words. Taking them clears them.
    pub fn take_agent_alerts(&mut self) -> Vec<Alert> {
        self.agents.radar.take_alerts()
    }

    /// What one pane event means for the radar. Returns whether the chrome shows something new.
    /// A known agent that appears, starts or stops a turn, or leaves is followed by its watch.
    pub(in crate::hub) fn agents_observe(&mut self, pane: PaneId, event: &PaneEvent) -> Changed {
        let now = self.agents.now();
        let was = self.agent_and_state(pane);
        let changed = self.agents.radar.observe(pane, now, event);
        let is = self.agent_and_state(pane);
        let (was_agent, is_agent) = (was.0.clone(), is.0.clone());
        self.watch_follows(pane, was, is);
        self.plugin_adapter_follows(pane, was_agent, is_agent);
        changed
    }

    fn agent_and_state(&self, pane: PaneId) -> (Option<String>, AgentState) {
        let agent = self.agents.radar.agent_of(pane).map(str::to_owned);
        (agent, self.agents.radar.state_of(pane))
    }

    /// The pane has the keyboard and the window is in use.
    pub fn pane_focused(&mut self, pane: PaneId) {
        let now = self.agents.now();
        self.agents.radar.set_focus(now, Some(pane));
    }

    /// The keyboard left the pane.
    pub fn pane_blurred(&mut self, pane: PaneId) {
        if self.agents.radar.focused() == Some(pane) {
            let now = self.agents.now();
            self.agents.radar.set_focus(now, None);
        }
    }

    /// After a workspace change: a pane that is no longer the keyboard pane is not focused.
    pub(in crate::hub) fn agents_after_act(&mut self) {
        let keyboard = self.keyboard_pane();
        if let Some(held) = self.agents.radar.focused()
            && keyboard != Some(held)
        {
            let now = self.agents.now();
            self.agents.radar.set_focus(now, None);
        }
        self.forget_closed_trees();
    }

    /// A pane's program ended and the pane is gone.
    pub(in crate::hub) fn agent_pane_ended(&mut self, pane: PaneId) {
        self.agents.radar.forget(pane);
        self.plugins.forget_pane(pane);
        self.agents.points.forget(pane);
        self.watch_pane_ended(pane);
        if let Some(sockets) = &self.agents.sockets {
            sockets.close(pane);
        }
    }

    /// The pane's status socket, opened for its program's `TEMOR_SOCKET`. A problem is told once;
    /// the pane starts without a socket.
    pub(in crate::hub) fn open_socket(&mut self, pane: PaneId) -> Option<PathBuf> {
        let sockets = self.agents.sockets.as_ref()?;
        match sockets.open(pane) {
            Ok(path) => Some(path),
            Err(err) => {
                if self.agents.socket_told == super::state::Told::Not {
                    self.agents.socket_told = super::state::Told::Once;
                    self.notices
                        .push(format!("Agents cannot report to this window: {err}"));
                }
                None
            }
        }
    }

    /// Time passed (about once a second): the radar settles and the worktrees due a look get
    /// one. Returns whether the chrome shows something new.
    pub fn agent_tick(&mut self) -> Changed {
        let now = self.agents.now();
        let changed = self.agents.radar.tick(now);
        self.look_at_due_trees(now);
        changed
    }

    /// ⌘⇧U: the next agent pane that waits for the person takes the keyboard.
    pub fn jump_to_waiting(&mut self) {
        let from = self.keyboard_pane();
        let Some(pane) = self.agents.radar.next_waiting(from) else {
            self.notices.push("No agent is waiting for you".to_owned());
            return;
        };
        let Some(tab) = self.ws.tab_of(pane) else {
            return;
        };
        self.act(WorkspaceIn::Tabs(TabsIn::Select(TabSel::Id(tab))));
        self.act(WorkspaceIn::Focus(pane));
    }

    /// What a tab shows: its most urgent pane's state.
    pub fn agent_state_of_tab(&self, tab: TabId) -> AgentState {
        let panes = self
            .ws
            .tabs
            .tab(tab)
            .map(|t| t.tree.panes())
            .unwrap_or_default();
        self.agents.radar.most_urgent(panes)
    }
}
