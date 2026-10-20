//! The hub's side of watched agents (PLAN P19, docket DESIGN §12). An agent program in a pane is
//! watched by a docket session that keeps restore points for it: a watch opens when the agent
//! shows up, a point is taken when it starts working, the turn ends when it stops, and the watch
//! closes when the agent or its pane goes. A session the assistant already holds (a window it
//! handed to the terminal) is never closed here, and one docket recognised as an app's
//! conversation stays open after the agent exits, until the pane ends. The calls run on worker
//! threads in the order they were wanted (see `watch`); a failure is written down once per pane
//! and not shown.

use std::sync::Arc;

use temor_core::agent::seen::CommandLine;
use temor_core::presence::AgentState;
use temor_core::vocab::ids::PaneId;
use temor_desktop::{DesktopError, TurnMark, WatchAsk, Watching};

use super::checkpoints::words;
use super::jobs::Done;
use super::watch::{Call, Closing, Edge, Need, Opening, Want, edge, rewind_for};
use crate::hub::model::Hub;

/// Whether the pane's restore points are read again after an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Refresh {
    Yes,
    No,
}

impl Hub {
    /// What a pane event did to `pane`'s agent and its state: the watch follows. `command_was` is
    /// the agent's command line before the event; one learned since may upgrade the watch.
    pub(super) fn watch_follows(
        &mut self,
        pane: PaneId,
        was: (Option<String>, AgentState),
        now: (Option<String>, AgentState),
        command_was: Option<CommandLine>,
    ) {
        let ((was_agent, before), (agent, after)) = (was, now);
        let Some(agent) = agent else {
            if was_agent.is_some() {
                self.leave_watch(pane);
            }
            return;
        };
        let turn = edge(before, after);
        let learned = self.agents.radar.command_of(pane) != command_was.as_ref();
        if was_agent.is_none() || turn == Some(Edge::Begin) || learned {
            self.open_watch(pane, &agent);
        }
        match turn {
            Some(Edge::Begin) => self.agents.watches.want(pane, Want::Mark),
            Some(Edge::End(how)) => self.agents.watches.want(pane, Want::End(how)),
            None => {}
        }
        self.pump_watch(pane);
    }

    /// Wants a session for `agent` in `pane`, unless it has one or no folder is known. A pane
    /// that still holds the session of an earlier, different agent closes it first, and a plain
    /// session opened before the agent's command line was known is opened again with it.
    fn open_watch(&mut self, pane: PaneId, agent: &str) {
        let Some(workspace) = self.pane_cwd(pane) else {
            return;
        };
        let title = self.ws.tab_of(pane).map(|tab| self.tab_name(tab));
        let label = match title.filter(|t| !t.is_empty()) {
            Some(title) => format!("{agent} in {title}"),
            None => agent.to_owned(),
        };
        let opening = Opening {
            program: agent.to_owned(),
            ask: WatchAsk {
                workspace,
                label,
                rewind: rewind_for(agent),
                seen: self.agents.radar.command_of(pane).cloned(),
            },
        };
        match self.agents.watches.need(pane, &opening) {
            Need::Nothing => {}
            Need::Open => self.agents.watches.want(pane, Want::Open(opening)),
            Need::Replace | Need::Upgrade => {
                self.agents.watches.want(pane, Want::Close(Closing::Always));
                self.agents.watches.want(pane, Want::Open(opening));
            }
        }
    }

    /// The agent left the pane: its watch closes unless the assistant holds it or it is a
    /// recognised app's, and its points are no longer listed.
    fn leave_watch(&mut self, pane: PaneId) {
        self.agents.watches.want(pane, Want::Close(Closing::Leave));
        self.pump_watch(pane);
    }

    /// The pane ended: the watch closes, and is forgotten once docket has been told.
    pub(super) fn watch_pane_ended(&mut self, pane: PaneId) {
        self.agents.watches.end_life(pane);
        self.agents.watches.want(pane, Want::Close(Closing::Always));
        self.pump_watch(pane);
    }

    /// Starts the next call wanted for `pane`, if the one before it has answered.
    fn pump_watch(&mut self, pane: PaneId) {
        let Some(call) = self.agents.watches.next(pane) else {
            return;
        };
        let port = Arc::clone(&self.agents.points.port);
        match call {
            Call::Open(ask) => self.agents.jobs.spawn(move || Done::WatchOpened {
                pane,
                result: port.watch(&ask),
            }),
            Call::Mark(session) => self.agents.jobs.spawn(move || Done::WatchMarked {
                pane,
                result: port.mark(&session),
            }),
            Call::End(session, turn, how) => self.agents.jobs.spawn(move || Done::WatchEnded {
                pane,
                result: port.turn_ended(&session, turn, how),
            }),
            // The Restore list stays after the agent exits (that is when undoing is wanted);
            // it goes when the pane ends (`agent_pane_ended`).
            Call::Close(session) => {
                self.agents.jobs.spawn(move || Done::WatchClosed {
                    pane,
                    result: port.close(&session),
                });
            }
        }
    }

    pub(super) fn watch_opened(&mut self, pane: PaneId, result: Result<Watching, DesktopError>) {
        self.agents.watches.answered(pane);
        match result {
            Ok(watching) => {
                let session = watching.session.clone();
                self.agents.watches.opened(pane, watching);
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
