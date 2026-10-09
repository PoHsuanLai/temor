use super::*;
use crate::digest::ContentHash;

fn hello(newest: u16, oldest: u16, caps: &[Capability]) -> Hello {
    Hello {
        protocol: ProtocolVersion(newest),
        oldest_protocol: ProtocolVersion(oldest),
        helper: HelperVersion(ContentHash([7; 32])),
        caps: caps.to_vec(),
    }
}

fn agreed(result: Negotiation) -> Agreement {
    match result {
        Negotiation::Agreed(agreement) => agreement,
        Negotiation::Incompatible { reason } => panic!("expected agreement, got {reason:?}"),
    }
}

/// Each row: (name, ours (newest, oldest), theirs (newest, oldest), capabilities both ends
/// offer, the agreed version). Each row is checked in both directions.
type AgreedCase = (
    &'static str,
    (u16, u16),
    (u16, u16),
    &'static [Capability],
    u16,
);

const AGREED_CASES: &[AgreedCase] = &[
    (
        "equal single versions",
        (3, 3),
        (3, 3),
        &[Capability::Sessions],
        3,
    ),
    ("overlapping ranges", (5, 2), (4, 3), &[], 4),
    ("touching ranges share the edge", (4, 4), (9, 4), &[], 4),
];

#[test]
fn the_agreed_version_is_the_newest_common() {
    for &(name, ours, theirs, caps, want) in AGREED_CASES {
        let ours = hello(ours.0, ours.1, caps);
        let theirs = hello(theirs.0, theirs.1, caps);
        for (a, b) in [(&ours, &theirs), (&theirs, &ours)] {
            let agreement = agreed(negotiate(a, b));
            assert_eq!(agreement.protocol, ProtocolVersion(want), "{name}");
            assert_eq!(agreement.caps, caps, "{name}");
        }
    }
}

#[test]
fn disjoint_ranges_are_incompatible_in_both_directions() {
    let newer = hello(7, 6, &[]);
    let older = hello(3, 1, &[]);
    for (ours, theirs) in [(&newer, &older), (&older, &newer)] {
        assert_eq!(
            negotiate(ours, theirs),
            Negotiation::Incompatible {
                reason: Incompatibility::NoCommonProtocol {
                    ours: ours.range(),
                    theirs: theirs.range(),
                },
            }
        );
    }
}

#[test]
fn an_inverted_peer_range_is_refused_even_when_it_would_overlap() {
    let ours = hello(5, 1, &[]);
    let theirs = hello(2, 4, &[]);
    assert_eq!(
        negotiate(&ours, &theirs),
        Negotiation::Incompatible {
            reason: Incompatibility::InvertedRange {
                theirs: theirs.range(),
            },
        }
    );
}

#[test]
fn capabilities_are_the_sorted_intersection_and_both_ends_agree_on_the_order() {
    let ours = hello(
        1,
        1,
        &[Capability::Trial, Capability::Files, Capability::Ports],
    );
    let theirs = hello(1, 1, &[Capability::Files, Capability::Trial]);
    let here = agreed(negotiate(&ours, &theirs)).caps;
    let there = agreed(negotiate(&theirs, &ours)).caps;
    assert_eq!(here, [Capability::Files, Capability::Trial]);
    assert_eq!(here, there);
}

#[test]
fn an_unknown_capability_is_never_agreed() {
    let ours = hello(1, 1, &[Capability::Unknown, Capability::Files]);
    let theirs = hello(1, 1, &[Capability::Unknown, Capability::Files]);
    assert_eq!(agreed(negotiate(&ours, &theirs)).caps, [Capability::Files]);
}

#[test]
fn the_agreement_carries_the_peers_helper_identity() {
    let ours = hello(1, 1, &[]);
    let mut theirs = hello(1, 1, &[]);
    theirs.helper = HelperVersion(ContentHash([9; 32]));
    assert_eq!(agreed(negotiate(&ours, &theirs)).helper, theirs.helper);
}
