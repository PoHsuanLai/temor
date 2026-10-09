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

#[test]
fn equal_single_versions_agree_on_that_version() {
    let ours = hello(3, 3, &[Capability::Sessions]);
    let theirs = hello(3, 3, &[Capability::Sessions]);
    let agreement = agreed(negotiate(&ours, &theirs));
    assert_eq!(agreement.protocol, ProtocolVersion(3));
    assert_eq!(agreement.caps, [Capability::Sessions]);
}

#[test]
fn overlapping_ranges_agree_on_the_newest_common_version() {
    let ours = hello(5, 2, &[]);
    let theirs = hello(4, 3, &[]);
    assert_eq!(
        agreed(negotiate(&ours, &theirs)).protocol,
        ProtocolVersion(4)
    );
    assert_eq!(
        agreed(negotiate(&theirs, &ours)).protocol,
        ProtocolVersion(4)
    );
}

#[test]
fn touching_ranges_agree_on_the_shared_edge() {
    let ours = hello(4, 4, &[]);
    let theirs = hello(9, 4, &[]);
    assert_eq!(
        agreed(negotiate(&ours, &theirs)).protocol,
        ProtocolVersion(4)
    );
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
