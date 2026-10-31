//! What keeps a helper session across the window's life (PLAN §9b, D18): the session numbers go
//! into the session file and come back on restore, quitting lets go of the links without
//! ending anything, a stalled link is dropped, and the window's colours follow to the host.

use temor_client::model::host::store::{HostSession, HostsFile};
use temor_client::remote::ids::RemoteSessionId;
use temor_client::remote::link::LinkEvent;

use super::model::Hub;
use super::remote::RemoteHub;
use temor_panes::remote::event::Generation;
use temor_panes::remote::link::lock;
use temor_panes::remote::palette::wire_palette;

/// The session to save for a pane: the one it has now, else the one it was saved with and has
/// not got back yet; none once the host lost it.
pub(super) fn session_to_save(
    live: Option<RemoteSessionId>,
    waiting: Option<RemoteSessionId>,
) -> Option<HostSession> {
    live.or(waiting).map(|session| HostSession(session.0))
}

impl RemoteHub {
    /// Puts each helper pane's session number in `hosts`.
    pub(super) fn note_sessions(&self, hosts: &mut HostsFile) {
        for saved in &mut hosts.panes {
            let live = self.panes.get(&saved.pane).and_then(|pane| pane.session);
            saved.session = session_to_save(live, self.saved.get(&saved.pane).copied());
        }
    }
}

impl Hub {
    /// What the session file keeps about hosts: the answers, which panes are on which host, and
    /// the helper session each of those panes has.
    pub(super) fn hosts_file(&self) -> HostsFile {
        let mut hosts = self.ssh.hosts_file();
        self.remote.note_sessions(&mut hosts);
        hosts
    }

    /// A ping went unanswered: the machine decides, and a link it drops is dialed again.
    pub(super) fn remote_stalled(
        &mut self,
        host: &temor_client::ssh::HostAlias,
        generation: Generation,
    ) {
        if !self.current(host, generation) {
            return;
        }
        let at = self.remote.now();
        self.step_link(host, LinkEvent::PingTimedOut { at });
        self.remote_tick_host(host);
    }

    /// Ends the `ssh` of a connection that stalled, so what waits on it fails; its panes stop
    /// sending until the next connection.
    pub(super) fn remote_drop_link(&mut self, host: &temor_client::ssh::HostAlias) {
        if let Some(line) = self.remote.lines.get_mut(host) {
            if let Some(conn) = line.conn.take() {
                conn.drop_link();
            }
            line.ports.clear();
        }
        for remote in self.remote.panes.values().filter(|p| p.alias == *host) {
            lock(&remote.target).conn = None;
        }
    }

    /// The window's colours changed: every open helper session gets them.
    pub(super) fn remote_palette(&self) {
        let palette = wire_palette(&self.palette);
        for (host, line) in &self.remote.lines {
            let Some(conn) = line.conn.clone() else {
                continue;
            };
            let sessions: Vec<RemoteSessionId> = self
                .remote
                .panes
                .values()
                .filter(|pane| pane.alias == *host)
                .filter_map(|pane| pane.session)
                .collect();
            if !sessions.is_empty() {
                self.remote.work.palette(conn, sessions, palette);
            }
        }
    }

    /// The window is quitting: the links go without a word to the sessions, so each host keeps
    /// its sessions (detached) for the next start. Closing a pane is what ends one.
    pub(super) fn remote_quit(&mut self) {
        for line in self.remote.lines.values_mut() {
            if let Some(conn) = line.conn.take() {
                conn.drop_link();
            }
        }
        for remote in self.remote.panes.values() {
            lock(&remote.target).conn = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pane_is_saved_with_its_live_session_else_the_one_it_has_yet_to_get_back() {
        let cases = [
            (
                Some(RemoteSessionId(4)),
                Some(RemoteSessionId(2)),
                Some(HostSession(4)),
            ),
            (Some(RemoteSessionId(4)), None, Some(HostSession(4))),
            (None, Some(RemoteSessionId(2)), Some(HostSession(2))),
            (None, None, None),
        ];
        for (live, waiting, want) in cases {
            assert_eq!(session_to_save(live, waiting), want, "{live:?} {waiting:?}");
        }
    }
}
