//! Starting the hub: the last session when there is one, else a first tab.

use temor_core::workspace::restore::launch_plan;
use temor_core::workspace::{KeepClosed, WorkspaceIn};

use super::model::Hub;
use crate::edge::{Startup, load_hosts, load_session, sidecar};

impl Hub {
    /// Restores the saved session (every tab's panes, in their folders, with their scrollback
    /// where a snapshot was kept) or opens a first tab.
    pub fn start(&mut self) {
        if let Some(dir) = self.state_dir.clone() {
            let hosts = load_hosts(&dir);
            self.ssh.book = hosts.book;
            match load_session(&dir, KeepClosed::DEFAULT) {
                Ok(Startup::Restored(ws)) if !ws.tabs.place.tabs.is_empty() => {
                    let plan = launch_plan(&ws);
                    self.ws = ws;
                    self.ssh.restored = hosts.panes;
                    for spawn in plan {
                        let snapshot = sidecar::take(&dir, spawn.pane);
                        self.open_pane(&spawn, snapshot);
                    }
                }
                Ok(Startup::Restored(_) | Startup::NoFile | Startup::Fresh(_)) => {}
                Err(err) => self.notices.push(err.to_string()),
            }
            // A snapshot is good for one launch only: pane ids are reused after a restore.
            sidecar::clear(&dir);
        }
        if self.ws.tabs.place.tabs.is_empty() {
            self.act(WorkspaceIn::NewTab { cwd: None });
        }
        self.refresh_probe();
    }
}
