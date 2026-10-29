//! What the views show of a host pane, read from the hub. The view's words are the client's
//! (`temor_client::ssh::view`).

use temor_client::model::host::target::host_title;
use temor_client::model::vocab::ids::PaneId;
use temor_client::ssh::view::{
    CheckRow, Direct, EditLine, ForwardLine, HostView, Stage, status_of,
};

use super::model::{Hub, HubRef};
use super::ssh::{Check, Phase};

impl Hub {
    /// What to show for `pane`, when it is a session on a host.
    pub fn host_view(&self, pane: PaneId) -> Option<HostView> {
        let host = self.ssh.panes.get(&pane)?;
        let line = self.ssh.lines.get(&host.alias);
        let stage = match host.phase {
            Phase::Asking(action) => Stage::Asking(action),
            Phase::Waiting | Phase::Connecting => Stage::Connecting,
            Phase::Up => Stage::Up,
            Phase::Offline => Stage::Offline,
            Phase::Failed => match self.has_session(pane) {
                true => Stage::Ended,
                false => Stage::Failed,
            },
        };
        let forwards = line
            .map(|line| {
                line.forwards
                    .remote_ports()
                    .into_iter()
                    .map(|remote| ForwardLine {
                        remote,
                        local: line.forwards.local_of(remote),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let edits = self
            .ssh
            .edits
            .iter()
            .filter(|(_, edit)| edit.host == host.alias)
            .map(|(id, edit)| EditLine {
                id: *id,
                name: format!("{}:{}", edit.host.as_str(), edit.path.as_str()),
                status: status_of(edit.state, edit.asking),
            })
            .collect();
        Some(HostView {
            host: host.alias.as_str().to_owned(),
            stage,
            check: match host.check {
                Check::Shown(_) => CheckRow::Waiting,
                Check::Nothing => CheckRow::Nothing,
            },
            direct: match line.is_some_and(|line| line.direct.is_some()) {
                true => Direct::Available,
                false => Direct::Unavailable,
            },
            forwards,
            edits,
        })
    }

    fn has_session(&self, pane: PaneId) -> bool {
        self.slots
            .get(&pane)
            .is_some_and(|slot| slot.borrow().session.is_some())
    }

    /// The title of a host pane: `host:folder`, the folder as the pane's shell last reported it.
    pub(super) fn ssh_title(&self, pane: PaneId) -> Option<String> {
        let host = self.ssh.panes.get(&pane)?;
        let reported = self
            .slots
            .get(&pane)
            .and_then(|slot| slot.borrow().session.as_ref().and_then(|s| s.cwd()))
            .or_else(|| {
                self.book
                    .ws
                    .cwds
                    .get(&pane)
                    .map(|cwd| cwd.as_str().to_owned())
            });
        Some(host_title(host.alias.as_str(), reported.as_deref()))
    }

    /// The folder on the host the pane is in, as the pane's shell last reported it.
    pub fn host_folder(&self, pane: PaneId) -> Option<String> {
        self.ssh.panes.get(&pane)?;
        self.book
            .ws
            .cwds
            .get(&pane)
            .map(|cwd| cwd.as_str().to_owned())
    }
}

impl HubRef {
    /// What to show for `pane`, when it is a session on a host.
    pub fn host_view(&self, pane: PaneId) -> Option<HostView> {
        self.0.borrow().host_view(pane)
    }
}
