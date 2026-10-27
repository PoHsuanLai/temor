//! What the views show of a pane on a helper (PLAN §9b, Tier B): the question about installing it,
//! the connection's state, and the ports the host is listening on with what can be done about
//! each. Plain data; the views draw it and send the person's choices back.

use temor_core::remote::HelperState;
use temor_core::vocab::ids::PaneId;
use temor_remote::{HostReach, PortAction, port_actions};

use super::model::{Hub, HubRef};
use super::remote::Shown;
use super::remote_source::{LinkShown, link_shown};
use temor_client::remote::hold::Typing;
use temor_panes::remote::link::lock;

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
pub(super) fn stage_of(helper: &HelperState, shown: Shown, link: LinkShown) -> RemoteStage {
    match (helper, shown, link) {
        (HelperState::Offered, _, _) => RemoteStage::Asking,
        (_, Shown::Gone, _) => RemoteStage::Ended,
        (_, _, LinkShown::Lost) => RemoteStage::Lost,
        (_, _, LinkShown::Reconnecting) => RemoteStage::Reconnecting,
        (_, Shown::Opening, LinkShown::Quiet) => RemoteStage::Preparing,
        (_, Shown::Attached, LinkShown::Quiet) => RemoteStage::Live,
    }
}

impl Hub {
    /// What to show for `pane`, when it is a session on a helper.
    pub fn remote_view(&self, pane: PaneId) -> Option<RemoteView> {
        let remote = self.remote.panes.get(&pane)?;
        let line = self.remote.lines.get(&remote.alias)?;
        let stage = stage_of(&line.helper, remote.shown, link_shown(&line.link));
        let reach = match self
            .ssh
            .lines
            .get(&remote.alias)
            .and_then(|line| line.direct.as_ref())
        {
            Some(direct) => HostReach::Tailnet {
                name: direct.as_str().to_owned(),
            },
            None => HostReach::Other,
        };
        let ports = match stage {
            RemoteStage::Live => line
                .ports
                .iter()
                .map(|listening| PortRow {
                    port: listening.port.0,
                    program: listening.process.clone(),
                    actions: port_actions(&reach, listening.port),
                })
                .collect(),
            _ => Vec::new(),
        };
        Some(RemoteView {
            host: remote.alias.as_str().to_owned(),
            stage,
            ports,
            typing: lock(&remote.target).held.typing(),
        })
    }
}

impl HubRef {
    /// What to show for `pane`, when it is a session on a helper.
    pub fn remote_view(&self, pane: PaneId) -> Option<RemoteView> {
        self.0.borrow().remote_view(pane)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stage_follows_the_question_then_the_session_then_the_connection() {
        let ready = HelperState::Ready(temor_core::vocab::text::HelperHash("a".repeat(64)));
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
