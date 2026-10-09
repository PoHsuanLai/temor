//! Version agreement: both ends compute the same answer from the same two greetings, and the
//! answer is the newest version in both ranges or a typed refusal.

use super::support::values::{capability, helper_version};
use proptest::prelude::*;
use temor_wire::hello::{Hello, Negotiation, ProtocolVersion, negotiate};

fn hello() -> impl Strategy<Value = Hello> {
    (
        0u16..64,
        0u16..64,
        helper_version(),
        prop::collection::vec(capability(), 0..5),
    )
        .prop_map(|(newest, oldest, helper, caps)| Hello {
            protocol: ProtocolVersion(newest),
            oldest_protocol: ProtocolVersion(oldest),
            helper,
            caps,
        })
}

proptest! {
    #[test]
    fn both_ends_agree_the_same_way(ours in hello(), theirs in hello()) {
        let here = negotiate(&ours, &theirs);
        let there = negotiate(&theirs, &ours);
        match (&here, &there) {
            (Negotiation::Agreed(a), Negotiation::Agreed(b)) => {
                prop_assert_eq!(a.protocol, b.protocol);
                prop_assert!(a.caps.iter().all(|cap| b.caps.contains(cap)));
                prop_assert!(b.caps.iter().all(|cap| a.caps.contains(cap)));
                prop_assert_eq!(a.caps.len(), b.caps.len());
            }
            (Negotiation::Incompatible { .. }, Negotiation::Incompatible { .. }) => {}
            _ => prop_assert!(false, "the two ends disagree: {here:?} vs {there:?}"),
        }
    }

    #[test]
    fn an_agreed_version_lies_inside_both_ranges(ours in hello(), theirs in hello()) {
        if let Negotiation::Agreed(agreed) = negotiate(&ours, &theirs) {
            prop_assert!(agreed.protocol >= ours.oldest_protocol && agreed.protocol <= ours.protocol);
            prop_assert!(agreed.protocol >= theirs.oldest_protocol && agreed.protocol <= theirs.protocol);
            prop_assert!(agreed.caps.iter().all(|cap| theirs.caps.contains(cap)));
        }
    }
}
