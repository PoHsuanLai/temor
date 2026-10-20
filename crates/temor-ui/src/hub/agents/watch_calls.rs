//! What the watch bookkeeping (`watch`) decides and hands out: what a newly seen agent needs of
//! its watch, the calls wanted of the port, and the calls ready to run.

use temor_desktop::{SessionKey, TurnHow, TurnMark, WatchAsk};

use super::watch::Closing;

/// What a new agent in a pane needs of its watch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Need {
    /// Open a session.
    Open,
    /// The pane keeps the session of an earlier, different agent (another program, or another
    /// command line of a recognised app): close it, then open one.
    Replace,
    /// The session was opened plain, before the agent's command line was known, the line is
    /// known now, and no turn runs: close it, then open one that can be recognised. Mid-turn the
    /// upgrade waits for the next turn, so the running turn ends on the session it was marked on.
    Upgrade,
    /// What is open serves, or an open is already on its way.
    Nothing,
}

/// A session wanted for `program`, asking `ask`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Opening {
    pub(super) program: String,
    pub(super) ask: WatchAsk,
}

/// A call wanted of the port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Want {
    Open(Opening),
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
