//! The reconnect machine over generated event sequences: it never schedules a wait past the cap,
//! never dials more than the policy allows, and once gone it stays gone.

use proptest::prelude::*;
use temor_wire::link::{
    Attempt, GoneReason, LinkEffect, LinkEvent, LinkState, Millis, Monotonic, RetryPolicy,
    link_step,
};

const POLICY: RetryPolicy = RetryPolicy::DEFAULT;

fn event() -> impl Strategy<Value = LinkEvent> {
    prop_oneof![
        (0u64..120_000).prop_map(|at| LinkEvent::Lost { at: Monotonic(at) }),
        (0u64..120_000).prop_map(|at| LinkEvent::Tick { at: Monotonic(at) }),
        (0u64..120_000).prop_map(|at| LinkEvent::Reattached { at: Monotonic(at) }),
        (0u64..120_000).prop_map(|at| LinkEvent::AttemptFailed { at: Monotonic(at) }),
        Just(LinkEvent::Close),
        Just(LinkEvent::Refused),
    ]
}

proptest! {
    #[test]
    fn waits_stay_within_the_cap_and_dials_within_the_policy(
        events in prop::collection::vec(event(), 0..200),
    ) {
        let mut state = LinkState::Connected { sessions: Vec::new() };
        let mut dials = 0u32;
        let mut was_gone: Option<GoneReason> = None;
        for event in events {
            let (next, effects) = link_step(state, event, &POLICY);
            if let LinkState::Backoff { delay, attempt, .. } = &next {
                prop_assert!(*delay <= POLICY.cap);
                prop_assert!(attempt.0 <= POLICY.attempts.0);
            }
            for effect in &effects {
                if let LinkEffect::Dial { .. } = effect {
                    dials += 1;
                }
            }
            if let LinkState::Gone { reason } = next {
                was_gone = Some(reason);
            } else {
                prop_assert!(was_gone.is_none(), "left Gone");
            }
            state = next;
        }
        prop_assert!(dials <= POLICY.attempts.0);
    }
}

#[test]
fn the_default_policy_is_immediate_then_doubles_to_its_cap() {
    let delays: Vec<Millis> = (1..=8).map(|n| POLICY.delay(Attempt(n))).collect();
    assert_eq!(
        delays,
        [
            Millis(0),
            Millis(1_000),
            Millis(2_000),
            Millis(4_000),
            Millis(8_000),
            Millis(16_000),
            Millis(32_000),
            Millis(60_000),
        ]
    );
}
