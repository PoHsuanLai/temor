//! The calls that keep a watched agent's session in step with the agent (PLAN P19, docket
//! DESIGN §12), as plain bookkeeping: which call is wanted next for each pane, and what is known
//! so far (the session, the turn). The port is blocking, so the hub runs one call per pane at a
//! time on a worker thread, in the order they were wanted; this keeps that order and the
//! answers, and never touches the port itself.

use std::collections::{BTreeMap, VecDeque};

use temor_core::agent::rewind::{self, rewind_of};
use temor_core::presence::AgentState;
use temor_core::vocab::ids::PaneId;
use temor_desktop::{Rewind, SessionKey, TurnHow, TurnMark};

/// What changed about a turn between two states of the same agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Edge {
    /// The agent started a turn.
    Begin,
    /// The agent's turn is over.
    End(TurnHow),
}

/// The turn edge between two states, if there is one. Waiting is the middle of a turn: coming
/// out of it into work is not a new turn.
pub(super) fn edge(before: AgentState, after: AgentState) -> Option<Edge> {
    use AgentState::{Done, Failed, Idle, Waiting, Working};
    match (before, after) {
        (Idle | Done(_) | Failed(_), Working) => Some(Edge::Begin),
        (Working | Waiting(_), Idle | Done(_)) => Some(Edge::End(TurnHow::Answered)),
        (Working | Waiting(_), Failed(_)) => Some(Edge::End(TurnHow::Failed)),
        _ => None,
    }
}

/// What docket is told about who keeps the history of the agent program `agent`.
pub(super) fn rewind_for(agent: &str) -> Rewind {
    match rewind_of(agent) {
        rewind::Rewind::Own => Rewind::Agent,
        rewind::Rewind::Docket => Rewind::Docket,
    }
}

/// A call wanted of the port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Want {
    Open {
        workspace: String,
        label: String,
        rewind: Rewind,
    },
    Mark,
    End(TurnHow),
    Close,
}

/// A call ready to run, with what it needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Call {
    Open {
        workspace: String,
        label: String,
        rewind: Rewind,
    },
    Mark(SessionKey),
    End(SessionKey, TurnMark, TurnHow),
    Close(SessionKey),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flight {
    Free,
    Busy,
}

/// Whether the pane is still there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Life {
    Live,
    /// The pane ended: the watch is closed, then forgotten.
    Ending,
}

/// Whether a failure was already written down for the pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Told {
    Not,
    Once,
}

#[derive(Debug)]
struct Watch {
    session: Option<SessionKey>,
    turn: Option<TurnMark>,
    queue: VecDeque<Want>,
    flight: Flight,
    life: Life,
    told: Told,
}

impl Watch {
    fn new() -> Watch {
        Watch {
            session: None,
            turn: None,
            queue: VecDeque::new(),
            flight: Flight::Free,
            life: Life::Live,
            told: Told::Not,
        }
    }
}

/// Every pane's watch.
#[derive(Debug, Default)]
pub(super) struct Watches {
    panes: BTreeMap<PaneId, Watch>,
}

impl Watches {
    /// Whether an agent just seen in `pane` still needs its session opened: none is open, and no
    /// open is on its way.
    pub(super) fn needs_open(&self, pane: PaneId) -> bool {
        let Some(watch) = self.panes.get(&pane) else {
            return true;
        };
        let queued = watch.queue.iter().any(|w| matches!(w, Want::Open { .. }));
        // An open in flight has no session yet and nothing queued; the flight tells it apart.
        watch.session.is_none() && !queued && watch.flight == Flight::Free
    }

    /// `want` is added to `pane`'s calls.
    pub(super) fn want(&mut self, pane: PaneId, want: Want) {
        self.panes
            .entry(pane)
            .or_insert_with(Watch::new)
            .queue
            .push_back(want);
    }

    /// The pane ended: whatever is open is closed after what is already wanted.
    pub(super) fn end_life(&mut self, pane: PaneId) {
        if let Some(watch) = self.panes.get_mut(&pane) {
            watch.life = Life::Ending;
        }
    }

    /// Whether the pane ended.
    pub(super) fn is_ending(&self, pane: PaneId) -> bool {
        self.panes
            .get(&pane)
            .is_some_and(|w| w.life == Life::Ending)
    }

    /// The next call for `pane` that can run now, if the one before it has answered. A call that
    /// cannot run (no session yet, no turn to end) is dropped.
    pub(super) fn next(&mut self, pane: PaneId) -> Option<Call> {
        let watch = self.panes.get_mut(&pane)?;
        if watch.flight == Flight::Busy {
            return None;
        }
        let mut found = None;
        while let Some(want) = watch.queue.pop_front() {
            found = match want {
                Want::Open {
                    workspace,
                    label,
                    rewind,
                } if watch.session.is_none() && watch.life == Life::Live => Some(Call::Open {
                    workspace,
                    label,
                    rewind,
                }),
                Want::Mark => watch.session.clone().map(Call::Mark),
                Want::End(how) => match (watch.session.clone(), watch.turn.take()) {
                    (Some(session), Some(turn)) => Some(Call::End(session, turn, how)),
                    _ => None,
                },
                Want::Close => {
                    watch.turn = None;
                    watch.session.take().map(Call::Close)
                }
                Want::Open { .. } => None,
            };
            if found.is_some() {
                watch.flight = Flight::Busy;
                break;
            }
        }
        if found.is_none() {
            self.forget_if_done(pane);
        }
        found
    }

    /// The call in flight for `pane` answered.
    pub(super) fn answered(&mut self, pane: PaneId) {
        if let Some(watch) = self.panes.get_mut(&pane) {
            watch.flight = Flight::Free;
        }
    }

    /// The session `pane`'s watch opened.
    pub(super) fn opened(&mut self, pane: PaneId, session: SessionKey) {
        if let Some(watch) = self.panes.get_mut(&pane) {
            watch.session = Some(session);
        }
    }

    /// The turn `pane`'s agent began.
    pub(super) fn marked(&mut self, pane: PaneId, turn: TurnMark) {
        if let Some(watch) = self.panes.get_mut(&pane) {
            watch.turn = Some(turn);
        }
    }

    /// Whether this is the first failure of `pane`'s watch: the first is written down, the rest
    /// are not.
    pub(super) fn first_failure(&mut self, pane: PaneId) -> bool {
        let Some(watch) = self.panes.get_mut(&pane) else {
            return false;
        };
        let first = watch.told == Told::Not;
        watch.told = Told::Once;
        first
    }

    /// A pane that ended and has nothing left to say to docket is forgotten.
    pub(super) fn forget_if_done(&mut self, pane: PaneId) {
        let done = self.panes.get(&pane).is_some_and(|w| {
            w.life == Life::Ending
                && w.flight == Flight::Free
                && w.queue.is_empty()
                && w.session.is_none()
        });
        if done {
            self.panes.remove(&pane);
        }
    }
}
