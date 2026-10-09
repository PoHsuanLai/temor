//! The machine's states, events and effects.

use super::policy::Attempt;
use crate::ids::SessionId;
use ds_core::word::Word;

/// A point on the caller's monotonic clock, in milliseconds since whatever origin the caller
/// picked. The step never reads a clock itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Monotonic(pub u64);

/// A length of time in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Millis(pub u64);

/// Where the link is.
///
/// A loss is an event, not a resting state: the step moves straight from `Connected` to
/// `Backoff`, so a person who sees "reconnecting" is seeing `Backoff` or `Reattaching`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkState {
    /// Connected, with these sessions on the host.
    Connected {
        /// The sessions to reattach if the link drops.
        sessions: Vec<SessionId>,
    },
    /// Waiting to dial again.
    Backoff {
        /// The sessions to reattach.
        sessions: Vec<SessionId>,
        /// The attempt this wait leads to.
        attempt: Attempt,
        /// How long the wait is.
        delay: Millis,
        /// When the wait ends.
        due: Monotonic,
    },
    /// Dialing, through the helper's handshake and session reattach.
    Reattaching {
        /// The sessions to reattach.
        sessions: Vec<SessionId>,
        /// The attempt in progress.
        attempt: Attempt,
    },
    /// No longer reconnecting.
    Gone {
        /// Why.
        reason: GoneReason,
    },
}

/// Why the link stopped reconnecting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum GoneReason {
    /// The person closed the connection.
    Closed,
    /// Every attempt in the policy failed.
    GaveUp,
}

/// What happened, as the caller saw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkEvent {
    /// The link dropped: the ssh process exited, the network changed, or the machine woke.
    Lost {
        /// When.
        at: Monotonic,
    },
    /// The timer the step asked for has fired (or time has passed; an early tick is ignored).
    Tick {
        /// When.
        at: Monotonic,
    },
    /// A dial succeeded: the helper's greeting was agreed and the link is up.
    Reattached {
        /// When.
        at: Monotonic,
    },
    /// A dial failed.
    AttemptFailed {
        /// When.
        at: Monotonic,
    },
    /// The sessions the host holds changed while the link was up or down.
    SessionsChanged {
        /// The sessions now.
        sessions: Vec<SessionId>,
    },
    /// The person closed the connection.
    Close,
}

/// What the caller must do. The step returns these; it never does them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkEffect {
    /// Deliver a `Tick` at or after this time.
    WakeAt(Monotonic),
    /// Dial the host and run the handshake, for this attempt.
    Dial {
        /// The attempt.
        attempt: Attempt,
    },
    /// Reattach these sessions on the new link, in this order.
    Attach {
        /// The sessions.
        sessions: Vec<SessionId>,
    },
}
