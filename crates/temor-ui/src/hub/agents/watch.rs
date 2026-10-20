//! The calls that keep a watched agent's session in step with the agent (PLAN P19, docket
//! DESIGN §12), as plain bookkeeping: which call is wanted next for each pane, and what is known
//! so far (the session, the turn). The port is blocking, so the hub runs one call per pane at a
//! time on a worker thread, in the order they were wanted; this keeps that order and the
//! answers, and never touches the port itself.

use std::collections::{BTreeMap, VecDeque};

use temor_core::agent::seen::CommandLine;
use temor_core::vocab::ids::PaneId;
use temor_desktop::{Named, SessionKey, TurnMark, WatchOwner, Watching};

pub(super) use super::turns::{Edge, edge, rewind_for};
pub(super) use super::watch_calls::{Call, Need, Opening, Want};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flight {
    Free,
    /// An open is on its way.
    Opening,
    /// A mark is on its way: a turn begins.
    Marking,
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
    /// The agent program the open session is for.
    program: String,
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
            program: String::new(),
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

    /// Whether a turn runs on the open session, or is about to: the last mark or end wanted
    /// says, else the call on its way and the turn known.
    fn mid_turn(&self) -> bool {
        let wanted = self.queue.iter().rev().find_map(|w| match w {
            Want::Mark => Some(true),
            Want::End(_) => Some(false),
            _ => None,
        });
        wanted.unwrap_or(self.turn.is_some() || self.flight == Flight::Marking)
    }

    /// What the open session needs when `program` is seen with `seen` as its command line. Another
    /// program replaces it, even with no line (one under `sudo` cannot be read); an unknown line
    /// changes nothing else. A recognised app's session stays after its agent exits: the same
    /// line picks it up, another replaces it. A plain session is upgraded once its line is known
    /// and no turn runs; one the assistant holds, or already named, never is.
    fn need_for(&self, program: &str, seen: Option<&CommandLine>) -> Need {
        if self.program != program {
            return Need::Replace;
        }
        let Some(seen) = seen else {
            return Need::Nothing;
        };
        match (self.seen.as_ref(), self.owner, self.named) {
            (Some(open), _, _) if open == seen => Need::Nothing,
            (None, WatchOwner::Terminal, Named::Unknown) if self.mid_turn() => Need::Nothing,
            (None, WatchOwner::Terminal, Named::Unknown) => Need::Upgrade,
            (Some(_), WatchOwner::Terminal, Named::App) => Need::Replace,
            _ => Need::Nothing,
        }
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
    /// What an agent just seen in `pane`, as `opening` asks, needs of its watch.
    pub(super) fn need(&self, pane: PaneId, opening: &Opening) -> Need {
        let Some(watch) = self.panes.get(&pane) else {
            return Need::Open;
        };
        let queued = watch.queue.iter().any(|w| matches!(w, Want::Open(_)));
        if queued || watch.flight == Flight::Opening {
            return Need::Nothing;
        }
        match &watch.session {
            None => Need::Open,
            Some(_) => watch.need_for(&opening.program, opening.ask.seen.as_ref()),
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
        if watch.flight != Flight::Free {
            return None;
        }
        let mut found = None;
        while let Some(want) = watch.queue.pop_front() {
            found = match want {
                Want::Open(opening) if watch.session.is_none() && watch.life == Life::Live => {
                    watch.program = opening.program;
                    watch.seen = opening.ask.seen.clone();
                    Some(Call::Open(opening.ask))
                }
                Want::Mark => watch.session.clone().map(Call::Mark),
                Want::End(how) => match (watch.session.clone(), watch.turn.take()) {
                    (Some(session), Some(turn)) => Some(Call::End(session, turn, how)),
                    _ => None,
                },
                Want::Close(closing) => watch.close_call(closing),
                Want::Open(_) => None,
            };
            if let Some(call) = &found {
                watch.flight = match call {
                    Call::Open(_) => Flight::Opening,
                    Call::Mark(_) => Flight::Marking,
                    Call::End(..) | Call::Close(_) => Flight::Busy,
                };
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
