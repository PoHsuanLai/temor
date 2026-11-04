use super::*;
use crate::ids::RemoteSessionId;

const POLICY: RetryPolicy = RetryPolicy {
    base: Millis(100),
    cap: Millis(500),
    attempts: Attempt(4),
};

fn sessions() -> Vec<RemoteSessionId> {
    vec![RemoteSessionId(1), RemoteSessionId(2)]
}

fn at(ms: u64) -> Monotonic {
    Monotonic(ms)
}

fn connected() -> LinkState {
    LinkState::Connected {
        sessions: sessions(),
    }
}

fn backoff(attempt: u32, due: u64, delay: u64) -> LinkState {
    LinkState::Backoff {
        sessions: sessions(),
        attempt: Attempt(attempt),
        delay: Millis(delay),
        due: at(due),
    }
}

fn reattaching(attempt: u32) -> LinkState {
    LinkState::Reattaching {
        sessions: sessions(),
        attempt: Attempt(attempt),
    }
}

fn gone(reason: GoneReason) -> LinkState {
    LinkState::Gone { reason }
}

#[test]
fn the_retry_delays_double_from_the_base_and_stop_at_the_cap() {
    let delays: Vec<u64> = (1..=6).map(|n| POLICY.delay(Attempt(n)).0).collect();
    assert_eq!(delays, [0, 100, 200, 400, 500, 500]);
}

#[test]
fn a_huge_attempt_number_does_not_overflow() {
    assert_eq!(POLICY.delay(Attempt(u32::MAX)), Millis(500));
}

#[test]
fn the_transition_table() {
    use LinkEffect::{Attach, Dial, DropLink, WakeAt};
    use LinkEvent::{
        AttemptFailed, Close, Lost, PingTimedOut, Reattached, Refused, SessionsChanged, Tick,
    };
    let cases: Vec<(&str, LinkState, LinkEvent, LinkState, Vec<LinkEffect>)> = vec![
        (
            "a loss schedules an immediate first dial",
            connected(),
            Lost { at: at(1_000) },
            backoff(1, 1_000, 0),
            vec![WakeAt(at(1_000))],
        ),
        (
            "an unanswered ping on a live link drops it and redials at once",
            connected(),
            PingTimedOut { at: at(5_000) },
            backoff(1, 5_000, 0),
            vec![DropLink, WakeAt(at(5_000))],
        ),
        (
            "a stale ping timeout while waiting changes nothing",
            backoff(2, 1_200, 100),
            PingTimedOut { at: at(1_100) },
            backoff(2, 1_200, 100),
            Vec::new(),
        ),
        (
            "a stale ping timeout while dialing changes nothing",
            reattaching(1),
            PingTimedOut { at: at(1_100) },
            reattaching(1),
            Vec::new(),
        ),
        (
            "a tick at the due time starts the dial",
            backoff(1, 1_000, 0),
            Tick { at: at(1_000) },
            reattaching(1),
            vec![Dial {
                attempt: Attempt(1),
            }],
        ),
        (
            "an early tick keeps waiting",
            backoff(2, 1_200, 100),
            Tick { at: at(1_100) },
            backoff(2, 1_200, 100),
            Vec::new(),
        ),
        (
            "a stray loss while waiting changes nothing",
            backoff(2, 1_200, 100),
            Lost { at: at(1_100) },
            backoff(2, 1_200, 100),
            Vec::new(),
        ),
        (
            "a successful dial reattaches every session",
            reattaching(1),
            Reattached { at: at(1_050) },
            connected(),
            vec![Attach {
                sessions: sessions(),
            }],
        ),
        (
            "a failed dial waits for the next attempt",
            reattaching(1),
            AttemptFailed { at: at(1_050) },
            backoff(2, 1_150, 100),
            vec![WakeAt(at(1_150))],
        ),
        (
            "a link lost while dialing is a failed dial",
            reattaching(2),
            Lost { at: at(2_000) },
            backoff(3, 2_200, 200),
            vec![WakeAt(at(2_200))],
        ),
        (
            "the last failed dial gives up",
            reattaching(4),
            AttemptFailed { at: at(3_000) },
            gone(GoneReason::GaveUp),
            Vec::new(),
        ),
        (
            "closing while connected is final",
            connected(),
            Close,
            gone(GoneReason::Closed),
            Vec::new(),
        ),
        (
            "closing while waiting is final",
            backoff(2, 1_200, 100),
            Close,
            gone(GoneReason::Closed),
            Vec::new(),
        ),
        (
            "a refusal while connected is final, with no retry",
            connected(),
            Refused,
            gone(GoneReason::Refused),
            Vec::new(),
        ),
        (
            "a refusal while dialing is final, with no retry",
            reattaching(2),
            Refused,
            gone(GoneReason::Refused),
            Vec::new(),
        ),
        (
            "a refusal while waiting is final, with no retry",
            backoff(2, 1_200, 100),
            Refused,
            gone(GoneReason::Refused),
            Vec::new(),
        ),
        (
            "closing while dialing is final",
            reattaching(3),
            Close,
            gone(GoneReason::Closed),
            Vec::new(),
        ),
        (
            "gone absorbs every event",
            gone(GoneReason::GaveUp),
            Lost { at: at(9) },
            gone(GoneReason::GaveUp),
            Vec::new(),
        ),
        (
            "a session change while connected is recorded",
            connected(),
            SessionsChanged {
                sessions: vec![RemoteSessionId(3)],
            },
            LinkState::Connected {
                sessions: vec![RemoteSessionId(3)],
            },
            Vec::new(),
        ),
        (
            "a session change while dialing is recorded for the attach",
            reattaching(1),
            SessionsChanged {
                sessions: vec![RemoteSessionId(3)],
            },
            LinkState::Reattaching {
                sessions: vec![RemoteSessionId(3)],
                attempt: Attempt(1),
            },
            Vec::new(),
        ),
    ];
    for (name, state, event, want_state, want_effects) in cases {
        let (got_state, got_effects) = link_step(state, event, &POLICY);
        assert_eq!(got_state, want_state, "{name}: state");
        assert_eq!(got_effects, want_effects, "{name}: effects");
    }
}

#[test]
fn a_link_that_keeps_failing_walks_the_whole_schedule_then_gives_up() {
    let (mut state, _) = link_step(connected(), LinkEvent::Lost { at: at(0) }, &POLICY);
    let mut now = 0;
    let mut dials = 0;
    while let LinkState::Backoff { due, .. } = state.clone() {
        now = due.0;
        let (dialing, effects) = link_step(state, LinkEvent::Tick { at: at(now) }, &POLICY);
        if effects
            .iter()
            .any(|effect| matches!(effect, LinkEffect::Dial { .. }))
        {
            dials += 1;
        }
        let (next, _) = link_step(dialing, LinkEvent::AttemptFailed { at: at(now) }, &POLICY);
        state = next;
    }
    assert_eq!(dials, 4);
    assert_eq!(state, gone(GoneReason::GaveUp));
    assert_eq!(now, 100 + 200 + 400);
}
