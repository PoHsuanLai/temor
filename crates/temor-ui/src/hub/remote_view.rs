//! What the views show of a pane on a helper (PLAN §9b, Tier B), read from the hub: the
//! question about installing it, the connection's state, and the ports the host is listening on.
//! The view's words are the client's (`temor_client::remote::session`).

use temor_client::model::vocab::ids::PaneId;
use temor_client::remote::session::{PortRow, RemoteStage, RemoteView, link_shown, stage_of};
use temor_client::remote::{HostReach, port_actions};

use super::model::{Hub, HubRef};
use temor_panes::remote::link::lock;

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
