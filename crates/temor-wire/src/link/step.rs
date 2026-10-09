//! The transitions. Each state has one handler; the top level routes events to it.

use super::model::{GoneReason, LinkEffect, LinkEvent, LinkState, Millis, Monotonic};
use super::policy::{Attempt, RetryPolicy};
use crate::ids::SessionId;

/// One step of the reconnect machine: the next state and what the caller must do.
pub fn link_step(
    state: LinkState,
    event: LinkEvent,
    policy: &RetryPolicy,
) -> (LinkState, Vec<LinkEffect>) {
    match (state, event) {
        (gone @ LinkState::Gone { .. }, _) => (gone, Vec::new()),
        (_, LinkEvent::Close) => (
            LinkState::Gone {
                reason: GoneReason::Closed,
            },
            Vec::new(),
        ),
        (_, LinkEvent::Refused) => (
            LinkState::Gone {
                reason: GoneReason::Refused,
            },
            Vec::new(),
        ),
        (LinkState::Connected { sessions }, event) => on_connected(sessions, event, policy),
        (
            LinkState::Backoff {
                sessions,
                attempt,
                delay,
                due,
            },
            event,
        ) => on_backoff(sessions, attempt, delay, due, event),
        (LinkState::Reattaching { sessions, attempt }, event) => {
            on_reattaching(sessions, attempt, event, policy)
        }
    }
}

fn on_connected(
    sessions: Vec<SessionId>,
    event: LinkEvent,
    policy: &RetryPolicy,
) -> (LinkState, Vec<LinkEffect>) {
    match event {
        LinkEvent::Lost { at } => schedule(sessions, Attempt(1), at, policy),
        LinkEvent::SessionsChanged { sessions } => (LinkState::Connected { sessions }, Vec::new()),
        // Ticks, dial outcomes from an earlier loss and the rest change nothing while connected.
        _ => (LinkState::Connected { sessions }, Vec::new()),
    }
}

fn on_backoff(
    sessions: Vec<SessionId>,
    attempt: Attempt,
    delay: Millis,
    due: Monotonic,
    event: LinkEvent,
) -> (LinkState, Vec<LinkEffect>) {
    match event {
        LinkEvent::Tick { at } if at >= due => (
            LinkState::Reattaching { sessions, attempt },
            vec![LinkEffect::Dial { attempt }],
        ),
        LinkEvent::SessionsChanged { sessions } => (
            LinkState::Backoff {
                sessions,
                attempt,
                delay,
                due,
            },
            Vec::new(),
        ),
        // An early tick, a stray loss or a stale outcome changes nothing: the wait goes on.
        _ => (
            LinkState::Backoff {
                sessions,
                attempt,
                delay,
                due,
            },
            Vec::new(),
        ),
    }
}

fn on_reattaching(
    sessions: Vec<SessionId>,
    attempt: Attempt,
    event: LinkEvent,
    policy: &RetryPolicy,
) -> (LinkState, Vec<LinkEffect>) {
    match event {
        LinkEvent::Reattached { .. } => (
            LinkState::Connected {
                sessions: sessions.clone(),
            },
            vec![LinkEffect::Attach { sessions }],
        ),
        // A link that dies while dialing is a failed dial.
        LinkEvent::AttemptFailed { at } | LinkEvent::Lost { at } => {
            if attempt.0 >= policy.attempts.0 {
                (
                    LinkState::Gone {
                        reason: GoneReason::GaveUp,
                    },
                    Vec::new(),
                )
            } else {
                schedule(sessions, attempt.next(), at, policy)
            }
        }
        LinkEvent::SessionsChanged { sessions } => {
            (LinkState::Reattaching { sessions, attempt }, Vec::new())
        }
        // A tick while dialing waits for the dial's own outcome.
        _ => (LinkState::Reattaching { sessions, attempt }, Vec::new()),
    }
}

/// The state that waits for `attempt`, from `now`, and the timer that wakes it.
fn schedule(
    sessions: Vec<SessionId>,
    attempt: Attempt,
    now: Monotonic,
    policy: &RetryPolicy,
) -> (LinkState, Vec<LinkEffect>) {
    let delay = policy.delay(attempt);
    let due = Monotonic(now.0.saturating_add(delay.0));
    (
        LinkState::Backoff {
            sessions,
            attempt,
            delay,
            due,
        },
        vec![LinkEffect::WakeAt(due)],
    )
}
