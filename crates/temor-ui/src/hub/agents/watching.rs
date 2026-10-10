//! The hub's side of watched agents (PLAN P19, docket DESIGN §12). An agent program in a pane is
//! watched by a docket session that keeps restore points for it: a watch opens when the agent
//! shows up, a point is taken when it starts working, the turn ends when it stops, and the watch
//! closes when the agent or its pane goes. The calls run on worker threads in the order they
//! were wanted (see `watch`); a failure is written down once per pane and not shown.

use std::sync::Arc;

use temor_core::presence::AgentState;
use temor_core::vocab::ids::PaneId;
use temor_desktop::{DesktopError, SessionKey, TurnMark};

use super::checkpoints::words;
use super::jobs::Done;
use super::watch::{Call, Edge, Want, edge, rewind_for};
use crate::hub::model::Hub;

/// Whether the pane's restore points are read again after an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Refresh {
    Yes,
    No,
}

impl Hub {
    /// What a pane event did to `pane`'s agent and its state: the watch follows.
    pub(super) fn watch_follows(
        &mut self,
        pane: PaneId,
        was: (Option<String>, AgentState),
        now: (Option<String>, AgentState),
    ) {
        let ((was_agent, before), (agent, after)) = (was, now);
        let Some(agent) = agent else {
            if was_agent.is_some() {
                self.close_watch(pane);
            }
            return;
        };
        let turn = edge(before, after);
        if was_agent.is_none() || turn == Some(Edge::Begin) {
            self.open_watch(pane, &agent);
        }
        match turn {
            Some(Edge::Begin) => self.agents.watches.want(pane, Want::Mark),
            Some(Edge::End(how)) => self.agents.watches.want(pane, Want::End(how)),
            None => {}
        }
        self.pump_watch(pane);
    }

    /// Wants a session for `agent` in `pane`, unless it has one or no folder is known.
    fn open_watch(&mut self, pane: PaneId, agent: &str) {
        if !self.agents.watches.needs_open(pane) {
            return;
        }
        let Some(workspace) = self.pane_cwd(pane) else {
            return;
        };
        let title = self.ws.tab_of(pane).map(|tab| self.tab_name(tab));
        let label = match title.filter(|t| !t.is_empty()) {
            Some(title) => format!("{agent} in {title}"),
            None => agent.to_owned(),
        };
        let rewind = rewind_for(agent);
        let open = Want::Open {
            workspace,
            label,
            rewind,
        };
        self.agents.watches.want(pane, open);
    }

    /// The agent left the pane: the watch closes and its points are no longer listed.
    fn close_watch(&mut self, pane: PaneId) {
        self.agents.watches.want(pane, Want::Close);
        self.pump_watch(pane);
    }

    /// The pane ended: the watch closes, and is forgotten once docket has been told.
    pub(super) fn watch_pane_ended(&mut self, pane: PaneId) {
        self.agents.watches.end_life(pane);
        self.agents.watches.want(pane, Want::Close);
        self.pump_watch(pane);
    }

    /// Starts the next call wanted for `pane`, if the one before it has answered.
    fn pump_watch(&mut self, pane: PaneId) {
        let Some(call) = self.agents.watches.next(pane) else {
            return;
        };
        let port = Arc::clone(&self.agents.points.port);
        match call {
            Call::Open {
                workspace,
                label,
                rewind,
            } => self.agents.jobs.spawn(move || Done::WatchOpened {
                pane,
                result: port.watch(&workspace, &label, rewind),
            }),
            Call::Mark(session) => self.agents.jobs.spawn(move || Done::WatchMarked {
                pane,
                result: port.mark(&session),
            }),
            Call::End(session, turn, how) => self.agents.jobs.spawn(move || Done::WatchEnded {
                pane,
                result: port.turn_ended(&session, turn, how),
            }),
            Call::Close(session) => {
                self.agents.points.forget(pane);
                self.agents.jobs.spawn(move || Done::WatchClosed {
                    pane,
                    result: port.close(&session),
                });
            }
        }
    }

    pub(super) fn watch_opened(&mut self, pane: PaneId, result: Result<SessionKey, DesktopError>) {
        self.agents.watches.answered(pane);
        match result {
            Ok(session) => {
                self.agents.watches.opened(pane, session.clone());
                // A pane that ended while the call ran is closed next, not listed.
                if !self.agents.watches.is_ending(pane) {
                    self.set_agent_session(pane, Some(session));
                }
            }
            Err(err) => self.watch_failed(pane, &err),
        }
        self.pump_watch(pane);
    }

    pub(super) fn watch_marked(&mut self, pane: PaneId, result: Result<TurnMark, DesktopError>) {
        self.agents.watches.answered(pane);
        match result {
            Ok(turn) => {
                self.agents.watches.marked(pane, turn);
                // docket now says a turn is running: Restore waits.
                self.refresh_points(pane);
            }
            Err(err) => self.watch_failed(pane, &err),
        }
        self.pump_watch(pane);
    }

    pub(super) fn watch_answered(
        &mut self,
        pane: PaneId,
        result: Result<(), DesktopError>,
        refresh: Refresh,
    ) {
        self.agents.watches.answered(pane);
        match result {
            Ok(()) if refresh == Refresh::Yes => self.refresh_points(pane),
            Ok(()) => {}
            Err(err) => self.watch_failed(pane, &err),
        }
        self.pump_watch(pane);
    }

    /// Writes the first failure of a pane's watch down; nothing is shown to the person. A build
    /// with no agent layer fails every call and is not worth a line.
    fn watch_failed(&mut self, pane: PaneId, err: &DesktopError) {
        if matches!(err, DesktopError::Unsupported { .. }) {
            return;
        }
        if self.agents.watches.first_failure(pane) {
            eprintln!(
                "temor: restore points are not kept for pane {}: {}",
                pane.0,
                words(err)
            );
        }
    }
}
