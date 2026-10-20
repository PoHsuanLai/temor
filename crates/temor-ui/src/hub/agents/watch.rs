//! The calls that keep a watched agent's session in step with the agent (PLAN P19, docket
//! DESIGN §12), as plain bookkeeping: which call is wanted next for each pane, and what is known
//! so far (the session, the turn). The port is blocking, so the hub runs one call per pane at a
//! time on a worker thread, in the order they were wanted; this keeps that order and the
//! answers, and never touches the port itself.

use std::collections::{BTreeMap, VecDeque};

use temor_core::agent::rewind::{self, rewind_of};
use temor_core::agent::seen::CommandLine;
use temor_core::presence::AgentState;
use temor_core::vocab::ids::PaneId;
use temor_desktop::{Named, Rewind, SessionKey, TurnHow, TurnMark, WatchAsk, WatchOwner, Watching};

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

/// When a watch's session is closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Closing {
    /// The agent left the pane. A session the assistant holds is only forgotten; one for a
    /// recognised app stays open until the pane ends (the conversation may be resumed); any
    /// other is closed.
    Leave,
    /// The pane ended, or another agent took the pane: a session the window opened is closed.
    /// One the assistant holds is still only forgotten.
    Always,
}

/// What a new agent in a pane needs of its watch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Need {
    /// Open a session.
    Open,
    /// The pane keeps the session of an earlier, different agent: close it, then open one.
    Replace,
    /// What is open serves, or an open is already on its way.
    Nothing,
}

/// A call wanted of the port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Want {
    Open(WatchAsk),
    Mark,
    End(TurnHow),
    Close(Closing),
}

/// A call ready to run, with what it needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Call {
    Open(WatchAsk),
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
    /// Who closes the session, and whether its app was recognised; read once it is open.
    owner: WatchOwner,
    named: Named,
    /// What the agent of the open session was started with.
    seen: Option<CommandLine>,
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
            owner: WatchOwner::Terminal,
            named: Named::Unknown,
            seen: None,
            turn: None,
            queue: VecDeque::new(),
            flight: Flight::Free,
            life: Life::Live,
            told: Told::Not,
        }
    }
}

impl Watch {
    /// Whether the session outlives its agent: the window opened it for a recognised app.
    fn kept_open(&self) -> bool {
        self.owner == WatchOwner::Terminal && self.named == Named::App
    }

    /// The call that closes the session when `closing` says so. A session the assistant holds is
    /// forgotten and never closed.
    fn close_call(&mut self, closing: Closing) -> Option<Call> {
        self.turn = None;
        if closing == Closing::Leave && self.kept_open() {
            return None;
        }
        let session = self.session.take()?;
        match self.owner {
            WatchOwner::Terminal => Some(Call::Close(session)),
            WatchOwner::Companion => None,
        }
    }
}

/// Every pane's watch.
#[derive(Debug, Default)]
pub(super) struct Watches {
    panes: BTreeMap<PaneId, Watch>,
}

impl Watches {
    /// What an agent just seen in `pane`, asking `ask`, needs of its watch.
    pub(super) fn need(&self, pane: PaneId, ask: &WatchAsk) -> Need {
        let Some(watch) = self.panes.get(&pane) else {
            return Need::Open;
        };
        let queued = watch.queue.iter().any(|w| matches!(w, Want::Open(_)));
        if queued || watch.flight == Flight::Busy {
            return Need::Nothing;
        }
        match &watch.session {
            None => Need::Open,
            // A recognised app's session stays after its agent exits: the same command picks
            // it up again, another one replaces it.
            Some(_) if watch.kept_open() && watch.seen != ask.seen => Need::Replace,
            Some(_) => Need::Nothing,
        }
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
                Want::Open(ask) if watch.session.is_none() && watch.life == Life::Live => {
                    watch.seen = ask.seen.clone();
                    Some(Call::Open(ask))
                }
                Want::Mark => watch.session.clone().map(Call::Mark),
                Want::End(how) => match (watch.session.clone(), watch.turn.take()) {
                    (Some(session), Some(turn)) => Some(Call::End(session, turn, how)),
                    _ => None,
                },
                Want::Close(closing) => watch.close_call(closing),
                Want::Open(_) => None,
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

    /// The watch `pane`'s open call made.
    pub(super) fn opened(&mut self, pane: PaneId, watching: Watching) {
        if let Some(watch) = self.panes.get_mut(&pane) {
            watch.session = Some(watching.session);
            watch.owner = watching.owner;
            watch.named = watching.named;
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
