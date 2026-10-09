//! The person's answer to an offer. Offers from a remote are never saved without one of these.

/// The person's answer to a remote offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Consent {
    Allow,
    Refuse,
}
