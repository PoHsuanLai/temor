//! Version negotiation: the newest version both sides speak, and what both can do.

use super::model::{Capability, Hello, HelperVersion, ProtocolRange, ProtocolVersion};

/// The result of the handshake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Negotiation {
    /// The two ends can talk. Both compute the same agreement from the same two greetings.
    Agreed(Agreement),
    /// The two ends cannot talk, and the reason says why.
    Incompatible {
        /// Why the greetings do not meet.
        reason: Incompatibility,
    },
}

/// What the ends agreed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agreement {
    /// The newest version both speak.
    pub protocol: ProtocolVersion,
    /// The peer's helper identity, so the caller can see whether the helper is the one it shipped.
    pub helper: HelperVersion,
    /// The capabilities both offer, sorted, so both ends hold the same vector.
    pub caps: Vec<Capability>,
}

/// Why two greetings cannot be agreed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Incompatibility {
    /// The two ranges do not overlap.
    NoCommonProtocol {
        /// The local side's range.
        ours: ProtocolRange,
        /// The peer's range.
        theirs: ProtocolRange,
    },
    /// The peer announced a range that is backwards (oldest after newest). Refused outright, so a
    /// broken peer cannot agree by accident.
    InvertedRange {
        /// The peer's range as it was sent.
        theirs: ProtocolRange,
    },
}

/// Agrees a protocol and capabilities from the local greeting and the peer's.
///
/// The protocol is the newest version inside both ranges. The capabilities are the local ones
/// the peer also offers, sorted and each once. Helper identity is passed through: whether it is
/// current is the caller's
/// decision, since an old helper is updated, not refused.
pub fn negotiate(ours: &Hello, theirs: &Hello) -> Negotiation {
    let their_range = theirs.range();
    if their_range.oldest > their_range.newest {
        return Negotiation::Incompatible {
            reason: Incompatibility::InvertedRange {
                theirs: their_range,
            },
        };
    }
    let oldest = ours.oldest_protocol.max(theirs.oldest_protocol);
    let newest = ours.protocol.min(theirs.protocol);
    if oldest > newest {
        return Negotiation::Incompatible {
            reason: Incompatibility::NoCommonProtocol {
                ours: ours.range(),
                theirs: their_range,
            },
        };
    }
    // Both ends compute the same set, sorted by the enum's order, so the agreement is identical
    // whichever side computes it. A capability this build does not know is never agreed.
    let mut caps: Vec<Capability> = ours
        .caps
        .iter()
        .copied()
        .filter(|cap| *cap != Capability::Unknown && theirs.caps.contains(cap))
        .collect();
    caps.sort_unstable();
    caps.dedup();
    Negotiation::Agreed(Agreement {
        protocol: newest,
        helper: theirs.helper,
        caps,
    })
}
