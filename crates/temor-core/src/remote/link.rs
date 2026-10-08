//! `link_step`: one host's connection, kept alive across drops, sleep and network changes by
//! reconnecting with backoff and reattaching every pane (§9b Tier B).
//!
//! A timed machine: it implements `ds_core`'s [`Machine`], asking to be woken when its backoff
//! ends, so a test drives it on a virtual clock.

use crate::vocab::ids::Attempt;
use ds_core::machine::{Elapsed, Machine};
use ds_core::time::stamp::Stamp;

/// Where a host's connection stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Link {
    /// The first connection is being made.
    Connecting,
    /// Connected, panes attached.
    Attached,
    /// Dropped; the next try is at `retry_at`.
    Detached {
        /// When it dropped.
        since: Stamp,
        /// Tries so far.
        attempt: Attempt,
        /// When to try again.
        retry_at: Stamp,
    },
    /// A reconnect is under way.
    Reattaching {
        /// When it dropped.
        since: Stamp,
        /// Which try this is.
        attempt: Attempt,
    },
    /// Given up; the person or the network coming back starts again.
    Lost,
}

/// What moves a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LinkIn {
    /// The helper said hello and sent its snapshot: the panes can be drawn exactly.
    Hello,
    /// The ssh process exited, or a connect failed.
    SshExited,
    /// The network changed.
    NetworkChanged,
    /// The computer woke from sleep.
    Woke,
    /// The person asked to try again.
    Retry,
    /// The backoff timer fired.
    Elapsed,
}

impl From<Elapsed> for LinkIn {
    fn from(_: Elapsed) -> LinkIn {
        LinkIn::Elapsed
    }
}

/// What a link wants done.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LinkOut {
    /// Start ssh and ask the helper to reattach every pane.
    Reconnect,
    /// Check the live connection (after a network change or a wake).
    Probe,
    /// Draw the panes from the helper's snapshot and resume the stream.
    Reattached,
    /// Show the panes as disconnected.
    ShowDetached,
    /// Show that reconnecting has stopped.
    GaveUp,
}

/// How a link backs off. Proposed values, so the caller reads them from settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LinkParams {
    /// The first wait, in milliseconds; each try doubles it.
    pub first_wait_ms: u64,
    /// The longest wait.
    pub longest_wait_ms: u64,
    /// Tries before giving up.
    pub most_attempts: Attempt,
}

impl Default for LinkParams {
    fn default() -> LinkParams {
        LinkParams {
            first_wait_ms: 500,
            longest_wait_ms: 30_000,
            most_attempts: Attempt(12),
        }
    }
}

impl LinkParams {
    /// The wait after try `attempt`.
    pub fn wait(&self, attempt: Attempt) -> u64 {
        let doubled = self.first_wait_ms.saturating_mul(1u64 << attempt.0.min(32));
        doubled.min(self.longest_wait_ms)
    }
}

impl Machine for Link {
    type In = LinkIn;
    type Out = LinkOut;
    type Params = LinkParams;
    type Ctx = ();

    fn step(self, input: LinkIn, at: Stamp, params: &LinkParams, _: &()) -> (Link, Vec<LinkOut>) {
        link_step(self, input, at, params)
    }

    fn wake(&self) -> Option<Stamp> {
        match self {
            Link::Detached { retry_at, .. } => Some(*retry_at),
            _ => None,
        }
    }
}

/// The link after `input` at `at`.
pub fn link_step(
    link: Link,
    input: LinkIn,
    at: Stamp,
    params: &LinkParams,
) -> (Link, Vec<LinkOut>) {
    use LinkIn as I;
    match (link, input) {
        (Link::Connecting | Link::Reattaching { .. }, I::Hello) => {
            (Link::Attached, vec![LinkOut::Reattached])
        }
        (Link::Connecting, I::SshExited) => (Link::Lost, vec![LinkOut::GaveUp]),
        (Link::Attached, I::SshExited) => {
            let next = Link::Detached {
                since: at,
                attempt: Attempt(0),
                retry_at: at.after(params.wait(Attempt(0))),
            };
            (next, vec![LinkOut::ShowDetached])
        }
        (Link::Attached, I::NetworkChanged | I::Woke) => (Link::Attached, vec![LinkOut::Probe]),
        (
            Link::Detached {
                since,
                attempt,
                retry_at,
            },
            I::Elapsed,
        ) if at >= retry_at => reattach(since, Attempt(attempt.0 + 1)),
        (Link::Detached { since, .. }, I::NetworkChanged | I::Woke | I::Retry) => {
            reattach(since, Attempt(1))
        }
        (Link::Reattaching { attempt, .. }, I::SshExited) if attempt >= params.most_attempts => {
            (Link::Lost, vec![LinkOut::GaveUp])
        }
        (Link::Reattaching { since, attempt }, I::SshExited) => {
            let next = Link::Detached {
                since,
                attempt,
                retry_at: at.after(params.wait(attempt)),
            };
            (next, vec![])
        }
        (Link::Lost, I::NetworkChanged | I::Woke | I::Retry) => reattach(at, Attempt(1)),
        (link, _) => (link, vec![]),
    }
}

fn reattach(since: Stamp, attempt: Attempt) -> (Link, Vec<LinkOut>) {
    (
        Link::Reattaching { since, attempt },
        vec![LinkOut::Reconnect],
    )
}
