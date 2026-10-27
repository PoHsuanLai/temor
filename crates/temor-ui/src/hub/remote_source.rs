//! Which kind of session a host pane gets, and how a connection's state reads (PLAN §9b).
//! Pure: the hub carries out what these decide.

use temor_client::model::remote::{Grant, HelperState};
use temor_client::remote::link::{GoneReason, LinkState};

/// Whether this temor carries a helper build to put on hosts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blob {
    Have,
    Missing,
}

/// What a host pane's session runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// A plain `ssh` in a terminal of this computer (Tier A).
    Plain,
    /// A session on the host's helper (Tier B): it survives the connection dropping.
    Helper,
}

/// The source for a host the person has answered `grant` about, whose helper stands at
/// `helper`. A host that was never asked is still a candidate: the question comes once the
/// connection is up and the host has been looked at.
pub fn session_source(grant: Grant, blob: Blob, helper: &HelperState) -> Source {
    match (blob, grant, helper) {
        (Blob::Missing, _, _) => Source::Plain,
        (_, Grant::Refused, _) => Source::Plain,
        (_, _, HelperState::Refused | HelperState::Unsupported) => Source::Plain,
        _ => Source::Helper,
    }
}

/// How a connection reads to the person.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkShown {
    /// Nothing to say: connected, or closed on purpose.
    Quiet,
    /// The connection dropped and temor is trying again.
    Reconnecting,
    /// temor stopped trying; the person can ask again.
    Lost,
}

/// What a link state means for the pane.
pub fn link_shown(link: &LinkState) -> LinkShown {
    match link {
        LinkState::Connected { .. } => LinkShown::Quiet,
        LinkState::Backoff { .. } | LinkState::Reattaching { .. } => LinkShown::Reconnecting,
        LinkState::Gone {
            reason: GoneReason::Closed,
        } => LinkShown::Quiet,
        LinkState::Gone {
            reason: GoneReason::GaveUp | GoneReason::Refused,
        } => LinkShown::Lost,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use temor_client::model::vocab::text::HelperHash;
    use temor_client::remote::link::{Attempt, Millis, Monotonic};

    #[test]
    fn a_host_gets_the_helper_unless_it_was_refused_cannot_run_it_or_there_is_none_to_put_there() {
        let ready = HelperState::Ready(HelperHash("a".repeat(64)));
        let cases = [
            (Grant::Ask, Blob::Have, HelperState::Absent, Source::Helper),
            (
                Grant::Allowed,
                Blob::Have,
                HelperState::Absent,
                Source::Helper,
            ),
            (Grant::Allowed, Blob::Have, ready, Source::Helper),
            (
                Grant::Allowed,
                Blob::Have,
                HelperState::Offered,
                Source::Helper,
            ),
            (
                Grant::Ask,
                Blob::Have,
                HelperState::Uploading,
                Source::Helper,
            ),
            (
                Grant::Refused,
                Blob::Have,
                HelperState::Absent,
                Source::Plain,
            ),
            (
                Grant::Allowed,
                Blob::Missing,
                HelperState::Absent,
                Source::Plain,
            ),
            (
                Grant::Allowed,
                Blob::Have,
                HelperState::Unsupported,
                Source::Plain,
            ),
            (Grant::Ask, Blob::Have, HelperState::Refused, Source::Plain),
        ];
        for (grant, blob, helper, want) in cases {
            assert_eq!(
                session_source(grant, blob, &helper),
                want,
                "{grant:?} {blob:?} {helper:?}"
            );
        }
    }

    #[test]
    fn a_dropped_connection_reads_as_reconnecting_then_lost() {
        let cases = [
            (LinkState::Connected { sessions: vec![] }, LinkShown::Quiet),
            (
                LinkState::Backoff {
                    sessions: vec![],
                    attempt: Attempt(2),
                    delay: Millis(1_000),
                    due: Monotonic(5_000),
                },
                LinkShown::Reconnecting,
            ),
            (
                LinkState::Reattaching {
                    sessions: vec![],
                    attempt: Attempt(3),
                },
                LinkShown::Reconnecting,
            ),
            (
                LinkState::Gone {
                    reason: GoneReason::GaveUp,
                },
                LinkShown::Lost,
            ),
            (
                LinkState::Gone {
                    reason: GoneReason::Refused,
                },
                LinkShown::Lost,
            ),
            (
                LinkState::Gone {
                    reason: GoneReason::Closed,
                },
                LinkShown::Quiet,
            ),
        ];
        for (link, want) in cases {
            assert_eq!(link_shown(&link), want, "{link:?}");
        }
    }
}
