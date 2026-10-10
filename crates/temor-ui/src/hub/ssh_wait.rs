//! Three small things a host pane needs from the hub: a file asked for before its host is
//! connected opens once it is (connect, then edit); a reopened tab keeps its host; and a host
//! pane that wants the person's eyes marks its tab unread.

use temor_core::vocab::ids::PaneId;
use temor_core::workspace::WorkspaceIn;
use temor_ssh::{HostAlias, RemoteFilePath};

use super::model::Hub;
use super::ssh::Phase;

/// How many ticks (about two minutes) a file waits for its host before it is given up.
const WAIT_TICKS: u32 = 400;

/// A file the person asked for on a host that was not connected yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WaitingEdit {
    pub host: HostAlias,
    pub path: RemoteFilePath,
    /// Ticks left before giving up.
    pub left: u32,
}

impl Hub {
    /// Connects to `host` (a tab of its own, unless a pane of it is already on the way) and
    /// opens `path` as soon as the connection is up.
    pub(super) fn edit_when_up(&mut self, host: HostAlias, path: RemoteFilePath) {
        let name = host.as_str().to_owned();
        if self.ssh.panes_on(&host).is_empty() {
            self.open_host_named(&name);
        }
        self.notices.push(format!(
            "Connecting to {name}; {} opens when it's ready",
            path.as_str()
        ));
        self.ssh.waiting.push(WaitingEdit {
            host,
            path,
            left: WAIT_TICKS,
        });
    }

    /// Opens the files whose host is connected now; drops those whose host gave up.
    pub(super) fn poll_waiting_edits(&mut self) {
        for mut wait in std::mem::take(&mut self.ssh.waiting) {
            wait.left = wait.left.saturating_sub(1);
            match self.edit_tier(&wait.host) {
                Ok(tier) => self.begin_edit(wait.host, wait.path, tier),
                Err(_) if wait.left > 0 && self.host_is_trying(&wait.host) => {
                    self.ssh.waiting.push(wait)
                }
                Err(_) => self.notices.push(format!(
                    "Couldn't connect to {}, so {} wasn't opened",
                    wait.host.as_str(),
                    wait.path.as_str()
                )),
            }
        }
    }

    /// Whether a pane of `host` has not given up.
    fn host_is_trying(&self, host: &HostAlias) -> bool {
        self.ssh
            .panes_on(host)
            .iter()
            .filter_map(|pane| self.ssh.panes.get(pane))
            .any(|pane| pane.phase != Phase::Failed)
    }

    /// A pane of `pane`'s host that closed inside a tab kept for reopening is remembered, so
    /// the reopened pane is a session on the same host.
    pub(super) fn ssh_note_closed(&mut self, pane: PaneId, host: HostAlias) {
        let in_closed_tab = self
            .ws
            .closed
            .tabs
            .iter()
            .any(|closed| closed.tab.tree.contains(pane));
        if in_closed_tab {
            self.ssh.closed.insert(pane, host);
        }
    }

    /// The host a pane coming back from the closed tabs was a session on.
    pub(super) fn ssh_reclaim(&mut self, pane: PaneId) -> Option<HostAlias> {
        self.ssh.closed.remove(&pane)
    }

    /// A host pane wants to be seen (a link to confirm, a question): its tab is marked unread
    /// unless it is in front.
    pub(super) fn host_attention(&mut self, panes: Vec<PaneId>) {
        for pane in panes {
            self.act(WorkspaceIn::Activity(pane));
        }
    }
}
