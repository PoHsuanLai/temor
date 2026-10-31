//! A pane's session on a reconnected link (PLAN §9b, Tier B): after a dial succeeds, each pane
//! gets its session back with its screen restored, and a session the host lost is told to the
//! person.

use std::sync::Arc;

use temor_client::model::vocab::ids::PaneId;
use temor_client::remote::ids::RemoteSessionId;
use temor_client::ssh::HostAlias;

use super::model::Hub;
use super::remote::Shown;
use temor_panes::remote::event::{AttachFault, Generation, Ticket, Wanted};
use temor_panes::remote::palette::wire_palette;

impl Hub {
    /// Reattaches the sessions `sessions` on the new connection, each pane getting its screen.
    pub(super) fn remote_reattach(&mut self, host: &HostAlias, sessions: &[RemoteSessionId]) {
        let Some(line) = self.remote.lines.get(host) else {
            return;
        };
        let Some(conn) = line.conn.clone() else {
            return;
        };
        let (generation, router) = (line.generation, line.router.clone());
        let palette = wire_palette(&self.palette);
        for (pane, remote) in &self.remote.panes {
            let Some(session) = remote.session.filter(|s| sessions.contains(s)) else {
                continue;
            };
            if remote.alias != *host {
                continue;
            }
            let ticket = Ticket {
                pane: *pane,
                palette,
                target: Arc::clone(&remote.target),
                feeder: remote.feeder.clone(),
                wanted: Wanted::Again(session),
            };
            self.remote.work.attach(
                Arc::clone(&conn),
                router.clone(),
                (host.clone(), generation),
                ticket,
            );
        }
    }

    pub(super) fn remote_attached(
        &mut self,
        host: &HostAlias,
        generation: Generation,
        pane: PaneId,
        session: RemoteSessionId,
    ) {
        if !self.current(host, generation) {
            return;
        }
        if let Some(remote) = self.remote.panes.get_mut(&pane) {
            remote.session = Some(session);
            remote.shown = Shown::Attached;
        }
        self.remote.saved.remove(&pane);
        self.tell_sessions(host);
    }

    pub(super) fn remote_attach_failed(
        &mut self,
        host: &HostAlias,
        generation: Generation,
        pane: PaneId,
        fault: AttachFault,
    ) {
        if !self.current(host, generation) {
            return;
        }
        match fault {
            AttachFault::Gone => {
                if let Some(remote) = self.remote.panes.get_mut(&pane) {
                    remote.shown = Shown::Gone;
                    remote.session = None;
                    // The pane says its program ended, with "Try again" beside it.
                    remote.feeder.end(None);
                }
                self.remote.saved.remove(&pane);
                self.notices
                    .push(format!("{} no longer has that session", host.as_str()));
                self.tell_sessions(host);
            }
            // The link is probably down; its closing is reported on its own.
            AttachFault::Failed => {}
        }
    }
}
