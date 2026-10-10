//! What the views show of a host pane, read from the hub: the question being asked, whether the
//! connection is up, the browser check, the forwards and the files being edited. Plain data; the
//! views draw it and send the person's choices back.

use temor_core::host::consent::BootstrapAction;
use temor_core::host::edit::{Edit, Found};
use temor_core::host::target::host_title;
use temor_core::vocab::ids::PaneId;
use temor_ssh::Port;

use super::model::{Hub, HubRef};
use super::ssh::{Check, Phase};
use crate::edge::ssh_work::EditId;

/// Where the host pane stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// The person is being asked this.
    Asking(BootstrapAction),
    /// Connecting: a password, a passphrase or a browser check may be asked for in the pane.
    Connecting,
    /// Connected.
    Up,
    /// The session ended; the pane offers to start another.
    Ended,
    /// The tailnet shows the computer offline; the host view offers to try anyway.
    Offline,
    /// The session never started (the connection could not be placed): the host view offers to
    /// try again.
    Failed,
}

/// Whether the host's ports can be opened without a forward.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direct {
    Available,
    Unavailable,
}

/// How the browser check stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckRow {
    /// The host wants the person to confirm in a browser: an Open button is shown.
    Waiting,
    Nothing,
}

/// A forwarded port. `local` is the port on this computer once the forward is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForwardLine {
    pub remote: Port,
    pub local: Option<Port>,
}

/// What an edit is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditStatus {
    Opening,
    /// Open in the editor; saves go to the host.
    Open,
    Saving,
    /// The host's file changed since it was fetched: the person chooses.
    Changed,
    /// The host's file was written again since it was fetched, yet its time and size read the
    /// same (only the helper can tell): the person chooses.
    Rewritten,
    /// The host's file was removed since it was fetched: the person chooses.
    Deleted,
    /// A call failed; saving again tries again.
    Stalled,
    Broken,
}

/// A file of the host open for editing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditLine {
    pub id: EditId,
    /// `host:/path`.
    pub name: String,
    pub status: EditStatus,
}

/// Everything the views show for one host pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostView {
    pub host: String,
    pub stage: Stage,
    pub check: CheckRow,
    pub direct: Direct,
    pub forwards: Vec<ForwardLine>,
    pub edits: Vec<EditLine>,
}

fn status_of(state: Edit, asking: Option<Found>) -> EditStatus {
    match (state, asking) {
        (Edit::Opening, _) => EditStatus::Opening,
        (Edit::Open { .. }, _) => EditStatus::Open,
        (Edit::Checking { .. } | Edit::Uploading { .. }, _) => EditStatus::Saving,
        (Edit::Conflict { .. }, Some(Found::Deleted)) => EditStatus::Deleted,
        (Edit::Conflict { base, .. }, Some(Found::Changed(now))) if base == now => {
            EditStatus::Rewritten
        }
        (Edit::Conflict { .. }, _) => EditStatus::Changed,
        (Edit::Stalled { .. }, _) => EditStatus::Stalled,
        (Edit::Broken(_) | Edit::Closed, _) => EditStatus::Broken,
    }
}

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
            .and_then(|slot| {
                slot.borrow()
                    .session
                    .as_ref()
                    .and_then(|s| s.cwd().map(str::to_owned))
            })
            .or_else(|| self.ws.cwds.get(&pane).map(|cwd| cwd.as_str().to_owned()));
        Some(host_title(host.alias.as_str(), reported.as_deref()))
    }

    /// The folder on the host the pane is in, as the pane's shell last reported it.
    pub fn host_folder(&self, pane: PaneId) -> Option<String> {
        self.ssh.panes.get(&pane)?;
        self.ws.cwds.get(&pane).map(|cwd| cwd.as_str().to_owned())
    }
}

impl HubRef {
    /// What to show for `pane`, when it is a session on a host.
    pub fn host_view(&self, pane: PaneId) -> Option<HostView> {
        self.0.borrow().host_view(pane)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use temor_ssh::{ByteCount, RemoteStat, UnixSeconds};

    #[test]
    fn a_conflict_reads_as_changed_rewritten_or_removed() {
        let stat = |secs, size| RemoteStat {
            mtime: UnixSeconds(secs),
            size: ByteCount(size),
        };
        let base = stat(10, 5);
        let conflict = |found| Edit::Conflict { base, found };
        let cases = [
            (
                "later time",
                Found::Changed(stat(11, 5)),
                EditStatus::Changed,
            ),
            (
                "other size",
                Found::Changed(stat(10, 6)),
                EditStatus::Changed,
            ),
            (
                "same time and size",
                Found::Changed(base),
                EditStatus::Rewritten,
            ),
            ("removed", Found::Deleted, EditStatus::Deleted),
        ];
        for (name, found, want) in cases {
            assert_eq!(status_of(conflict(found), Some(found)), want, "{name}");
        }
    }
}
