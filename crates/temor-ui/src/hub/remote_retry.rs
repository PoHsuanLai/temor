//! The person's Retry on a connection temor gave up on (PLAN §9b, Tier B). The shared
//! connection may be gone for good, and a dial that can't ask anything can't make a new one, so
//! Retry runs the same step the first connect does: an `ssh` in the pane, where a password or a
//! key prompt shows. When it ends well the pane becomes a session on the helper again, and the
//! session it had is asked for, not a new one.

use temor_client::model::vocab::ids::PaneId;
use temor_client::remote::ids::SessionId;
use temor_client::remote::link::{Attempt, LinkState};
use temor_client::ssh::HostAlias;

use super::model::Hub;
use super::remote::Shown;
use super::ssh::Phase;

/// What pressing Retry does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RetryPlan {
    /// Connect again in the pane.
    Connect,
    /// Nothing: temor is still trying, or the pane is already connecting.
    Wait,
}

/// The plan for a pane whose host connection reads `link`, and whose own session stands at
/// `phase` (none for a pane that is not a host session).
pub(super) fn retry_plan(link: &LinkState, phase: Option<Phase>) -> RetryPlan {
    match (link, phase) {
        (_, None | Some(Phase::Connecting)) => RetryPlan::Wait,
        (LinkState::Gone { .. }, Some(_)) => RetryPlan::Connect,
        _ => RetryPlan::Wait,
    }
}

/// How a pane becomes a session on the helper once the shared connection is up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Begin {
    /// A new session on the host.
    Fresh,
    /// The session the pane had before the link was lost, to be attached again.
    Rejoin(SessionId),
}

/// How a pane begins, given the session it had (if it was one) and where that stood.
pub(super) fn begin_plan(before: Option<(Option<SessionId>, Shown)>) -> Begin {
    match before {
        Some((Some(session), Shown::Opening | Shown::Attached)) => Begin::Rejoin(session),
        _ => Begin::Fresh,
    }
}

impl Hub {
    /// The person pressed Retry on a connection temor gave up on.
    pub fn remote_retry(&mut self, pane: PaneId) {
        let Some(host) = self.alias_of(pane) else {
            return;
        };
        let phase = self.ssh.panes.get(&pane).map(|p| p.phase);
        let Some(line) = self.remote.lines.get(&host) else {
            return;
        };
        if retry_plan(&line.link, phase) == RetryPlan::Connect {
            self.start_connect(&host, pane);
        }
    }

    /// The shared connection is up again: dial the helper and reattach every session of `host`.
    pub(super) fn remote_rejoin(&mut self, host: &HostAlias) {
        let sessions: Vec<SessionId> = self
            .remote
            .panes
            .values()
            .filter(|p| p.alias == *host)
            .filter_map(|p| p.session)
            .collect();
        let Some(line) = self.remote.lines.get_mut(host) else {
            return;
        };
        line.link = LinkState::Reattaching {
            sessions,
            attempt: Attempt(1),
        };
        self.remote_dial(host);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use temor_client::model::host::consent::BootstrapAction;
    use temor_client::remote::link::{GoneReason, Millis, Monotonic};

    #[test]
    fn retry_connects_in_the_pane_only_for_a_lost_connection_that_is_not_already_connecting() {
        let gone = |reason| LinkState::Gone { reason };
        let waiting = LinkState::Backoff {
            sessions: vec![],
            attempt: Attempt(2),
            delay: Millis(1_000),
            due: Monotonic(5_000),
        };
        let cases = [
            (
                gone(GoneReason::GaveUp),
                Some(Phase::Up),
                RetryPlan::Connect,
            ),
            (
                gone(GoneReason::GaveUp),
                Some(Phase::Failed),
                RetryPlan::Connect,
            ),
            (
                gone(GoneReason::Refused),
                Some(Phase::Up),
                RetryPlan::Connect,
            ),
            (
                gone(GoneReason::GaveUp),
                Some(Phase::Connecting),
                RetryPlan::Wait,
            ),
            (gone(GoneReason::GaveUp), None, RetryPlan::Wait),
            (waiting, Some(Phase::Up), RetryPlan::Wait),
            (
                LinkState::Reattaching {
                    sessions: vec![],
                    attempt: Attempt(1),
                },
                Some(Phase::Up),
                RetryPlan::Wait,
            ),
            (
                LinkState::Connected { sessions: vec![] },
                Some(Phase::Asking(BootstrapAction::Integrate)),
                RetryPlan::Wait,
            ),
        ];
        for (link, phase, want) in cases {
            assert_eq!(retry_plan(&link, phase), want, "{link:?} {phase:?}");
        }
    }

    #[test]
    fn a_pane_that_had_a_live_session_asks_for_it_again_else_starts_a_new_one() {
        let session = SessionId(7);
        let cases = [
            (None, Begin::Fresh),
            (Some((None, Shown::Opening)), Begin::Fresh),
            (
                Some((Some(session), Shown::Attached)),
                Begin::Rejoin(session),
            ),
            (
                Some((Some(session), Shown::Opening)),
                Begin::Rejoin(session),
            ),
            (Some((Some(session), Shown::Gone)), Begin::Fresh),
        ];
        for (before, want) in cases {
            assert_eq!(begin_plan(before), want, "{before:?}");
        }
    }
}
