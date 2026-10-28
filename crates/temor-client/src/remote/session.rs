//! A pane on a host's helper, read for the person (PLAN §9b): which kind of session a host pane
//! gets, how its connection and its session read, and what the views show of it. Pure: the
//! window carries out what these decide and draws what they say.

use crate::model::remote::{Grant, HelperState};
use crate::remote::PortAction;
use crate::remote::hold::Typing;
use crate::remote::link::{GoneReason, LinkState};

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
pub fn session_source(grant: Grant, blob: bool, helper: &HelperState) -> Source {
    match (blob, grant, helper) {
        (false, _, _) => Source::Plain,
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

/// Where a pane's session stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shown {
    /// Waiting for the helper, or for the host to open the session.
    Opening,
    /// Open on the host and showing its screen.
    Attached,
    /// The host no longer has the session.
    Gone,
}

/// Where a pane on a helper stands, for the person.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteStage {
    /// The person is asked whether the helper may be put on the host.
    Asking,
    /// The helper is being put there, or the session is being opened.
    Preparing,
    /// The session is open.
    Live,
    /// The connection dropped; temor is trying again.
    Reconnecting,
    /// temor stopped trying; the person can ask again.
    Lost,
    /// The host no longer has the session.
    Ended,
}

/// A port a program on the host listens on, and what can be done about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortRow {
    pub port: u16,
    /// The program that listens, as the host shows it.
    pub program: String,
    /// Best first.
    pub actions: Vec<PortAction>,
}

/// Everything the views show for one pane on a helper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteView {
    pub host: String,
    pub stage: RemoteStage,
    pub ports: Vec<PortRow>,
    /// Whether typing was lost while the connection was down.
    pub typing: Typing,
}

/// The stage for a pane whose session is `shown` on a host whose helper stands at `helper`
/// and whose connection reads `link`.
pub fn stage_of(helper: &HelperState, shown: Shown, link: LinkShown) -> RemoteStage {
    match (helper, shown, link) {
        (HelperState::Offered, _, _) => RemoteStage::Asking,
        (_, Shown::Gone, _) => RemoteStage::Ended,
        (_, _, LinkShown::Lost) => RemoteStage::Lost,
        (_, _, LinkShown::Reconnecting) => RemoteStage::Reconnecting,
        (_, Shown::Opening, LinkShown::Quiet) => RemoteStage::Preparing,
        (_, Shown::Attached, LinkShown::Quiet) => RemoteStage::Live,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::vocab::text::HelperHash;
    use crate::remote::link::{Attempt, Millis, Monotonic};

    #[test]
    fn a_host_gets_the_helper_unless_it_was_refused_cannot_run_it_or_there_is_none_to_put_there() {
        let ready = HelperState::Ready(HelperHash("a".repeat(64)));
        let cases = [
            (Grant::Ask, true, HelperState::Absent, Source::Helper),
            (Grant::Allowed, true, HelperState::Absent, Source::Helper),
            (Grant::Allowed, true, ready, Source::Helper),
            (Grant::Allowed, true, HelperState::Offered, Source::Helper),
            (Grant::Ask, true, HelperState::Uploading, Source::Helper),
            (Grant::Refused, true, HelperState::Absent, Source::Plain),
            (Grant::Allowed, false, HelperState::Absent, Source::Plain),
            (
                Grant::Allowed,
                true,
                HelperState::Unsupported,
                Source::Plain,
            ),
            (Grant::Ask, true, HelperState::Refused, Source::Plain),
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

    #[test]
    fn the_stage_follows_the_question_then_the_session_then_the_connection() {
        let ready = HelperState::Ready(crate::model::vocab::text::HelperHash("a".repeat(64)));
        let cases = [
            (
                HelperState::Offered,
                Shown::Opening,
                LinkShown::Quiet,
                RemoteStage::Asking,
            ),
            (
                HelperState::Uploading,
                Shown::Opening,
                LinkShown::Quiet,
                RemoteStage::Preparing,
            ),
            (
                ready.clone(),
                Shown::Opening,
                LinkShown::Quiet,
                RemoteStage::Preparing,
            ),
            (
                ready.clone(),
                Shown::Attached,
                LinkShown::Quiet,
                RemoteStage::Live,
            ),
            (
                ready.clone(),
                Shown::Attached,
                LinkShown::Reconnecting,
                RemoteStage::Reconnecting,
            ),
            (
                ready.clone(),
                Shown::Attached,
                LinkShown::Lost,
                RemoteStage::Lost,
            ),
            (
                ready.clone(),
                Shown::Gone,
                LinkShown::Quiet,
                RemoteStage::Ended,
            ),
            (ready, Shown::Gone, LinkShown::Lost, RemoteStage::Ended),
        ];
        for (helper, shown, link, want) in cases {
            assert_eq!(
                stage_of(&helper, shown, link),
                want,
                "{helper:?} {shown:?} {link:?}"
            );
        }
    }
}
