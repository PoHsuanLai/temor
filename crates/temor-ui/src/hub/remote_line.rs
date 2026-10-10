//! A helper connection's life after the first dial (PLAN §9b, Tier B): the connection drops, temor
//! waits and dials again with backoff, and every session on it is reattached with its screen
//! restored. `link_step` decides; this runs what it names.

use std::sync::Arc;

use temor_core::vocab::ids::PaneId;
use temor_ssh::HostAlias;
use temor_wire::ids::SessionId;
use temor_wire::link::{Attempt, LinkEffect, LinkEvent, LinkState, RetryPolicy, link_step};
use temor_wire::message::ListeningPort;

use super::model::Hub;
use super::remote::{Busy, Reach, Shown};
use super::ssh_events::Redraw;
use crate::edge::remote_conn::{Conn, DialFault};
use crate::edge::remote_event::{AttachFault, Generation, Ticket, Wanted};
use crate::edge::remote_link::lock;
use crate::edge::remote_palette::wire_palette;

impl Hub {
    /// Whether an event about `host` is about its current connection.
    pub(super) fn current(&self, host: &HostAlias, generation: Generation) -> bool {
        self.remote
            .lines
            .get(host)
            .is_some_and(|line| line.generation == generation)
    }

    /// Moves `host`'s connection along and does what its machine asks.
    pub(super) fn step_link(&mut self, host: &HostAlias, event: LinkEvent) {
        let Some(line) = self.remote.lines.get_mut(host) else {
            return;
        };
        let state = std::mem::replace(
            &mut line.link,
            LinkState::Connected {
                sessions: Vec::new(),
            },
        );
        let (next, effects) = link_step(state, event, &RetryPolicy::DEFAULT);
        line.link = next;
        for effect in effects {
            match effect {
                // The tick polls for the end of a wait.
                LinkEffect::WakeAt(_) => {}
                LinkEffect::DropLink => self.remote_drop_link(host),
                LinkEffect::Dial { .. } => self.remote_dial(host),
                LinkEffect::Attach { sessions } => self.remote_reattach(host, &sessions),
                // An effect a later wire adds has nothing for the window to do yet.
                _ => {}
            }
        }
    }

    /// The sessions the host holds for this window's panes, for the machine to reattach.
    fn tell_sessions(&mut self, host: &HostAlias) {
        let sessions: Vec<SessionId> = self
            .remote
            .panes
            .values()
            .filter(|pane| pane.alias == *host)
            .filter_map(|pane| pane.session)
            .collect();
        self.step_link(host, LinkEvent::SessionsChanged { sessions });
        // The session file keeps the numbers; they changed.
        self.pace.changed();
    }

    pub(super) fn remote_dialed(
        &mut self,
        host: &HostAlias,
        generation: Generation,
        outcome: Result<Arc<Conn>, DialFault>,
    ) {
        if !self.current(host, generation) {
            return;
        }
        let at = self.remote.now();
        let reach = self
            .remote
            .lines
            .get(host)
            .map_or(Reach::Never, |l| l.reach);
        if let Some(line) = self.remote.lines.get_mut(host) {
            line.busy = Busy::Idle;
        }
        match (outcome, reach) {
            (Ok(conn), _) => {
                if let Some(line) = self.remote.lines.get_mut(host) {
                    line.conn = Some(conn);
                    line.reach = Reach::Reached;
                }
                self.step_link(host, LinkEvent::Reattached { at });
                self.remote_open_waiting(host);
            }
            (Err(fault), Reach::Never) => {
                self.notices.push(never_words(host, &fault));
                if let Some(line) = self.remote.lines.get_mut(host) {
                    line.helper = temor_core::remote::HelperState::Unsupported;
                }
                self.remote_fallback_all(host);
            }
            (Err(DialFault::Refused), Reach::Reached) => self.step_link(host, LinkEvent::Refused),
            (Err(_), Reach::Reached) => {
                self.step_link(host, LinkEvent::AttemptFailed { at });
                self.remote_tick_host(host);
            }
        }
    }

    /// Reattaches the sessions `sessions` on the new connection, each pane getting its screen.
    pub(super) fn remote_reattach(&mut self, host: &HostAlias, sessions: &[SessionId]) {
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
        session: SessionId,
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

    pub(super) fn remote_ports(
        &mut self,
        host: &HostAlias,
        generation: Generation,
        ports: Vec<ListeningPort>,
    ) {
        if self.current(host, generation)
            && let Some(line) = self.remote.lines.get_mut(host)
        {
            line.ports = ports;
        }
    }

    pub(super) fn remote_exited(
        &mut self,
        host: &HostAlias,
        generation: Generation,
        session: SessionId,
    ) {
        if !self.current(host, generation) {
            return;
        }
        for remote in self.remote.panes.values_mut() {
            if remote.alias == *host && remote.session == Some(session) {
                remote.session = None;
                remote.shown = Shown::Gone;
            }
        }
        self.tell_sessions(host);
    }

    /// The connection ended: the panes stop sending, and the machine starts waiting to dial.
    pub(super) fn remote_closed(&mut self, host: &HostAlias, generation: Generation) {
        if !self.current(host, generation) {
            return;
        }
        let at = self.remote.now();
        if let Some(line) = self.remote.lines.get_mut(host) {
            line.conn = None;
            line.ports.clear();
            line.busy = Busy::Idle;
        }
        for remote in self.remote.panes.values().filter(|p| p.alias == *host) {
            lock(&remote.target).conn = None;
        }
        self.step_link(host, LinkEvent::Lost { at });
        self.remote_tick_host(host);
    }

    /// Lets a wait that has run out end.
    pub(super) fn remote_tick_host(&mut self, host: &HostAlias) {
        let at = self.remote.now();
        self.step_link(host, LinkEvent::Tick { at });
    }

    /// One tick: connections that are waiting dial when their wait is over.
    pub(super) fn remote_tick(&mut self) -> Redraw {
        let waiting: Vec<HostAlias> = self
            .remote
            .lines
            .iter()
            .filter(|(_, line)| matches!(line.link, LinkState::Backoff { .. }))
            .map(|(host, _)| host.clone())
            .collect();
        let mut redraw = Redraw::No;
        for host in waiting {
            let before = self.remote.lines.get(&host).map(|line| line.generation);
            self.remote_tick_host(&host);
            if self.remote.lines.get(&host).map(|line| line.generation) != before {
                redraw = Redraw::Yes;
            }
        }
        redraw
    }

    /// The person pressed Retry on a connection temor gave up on.
    pub fn remote_retry(&mut self, pane: PaneId) {
        let Some(host) = self.remote.panes.get(&pane).map(|p| p.alias.clone()) else {
            return;
        };
        let sessions: Vec<SessionId> = self
            .remote
            .panes
            .values()
            .filter(|p| p.alias == host)
            .filter_map(|p| p.session)
            .collect();
        let Some(line) = self.remote.lines.get_mut(&host) else {
            return;
        };
        if matches!(line.link, LinkState::Gone { .. }) {
            line.link = LinkState::Reattaching {
                sessions,
                attempt: Attempt(1),
            };
            self.remote_dial(&host);
        }
    }
}

/// What the person is told when the helper cannot be reached the first time.
fn never_words(host: &HostAlias, fault: &DialFault) -> String {
    let name = host.as_str();
    match fault {
        DialFault::Refused => {
            format!("{name} has an older helper than this temor; using a plain connection")
        }
        DialFault::Spawn(_) | DialFault::Failed => {
            format!("The helper on {name} didn't start; using a plain connection")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_first_connection_that_fails_says_why_in_plain_words() {
        let host = HostAlias::new("prod").expect("a name");
        let cases = [
            (DialFault::Refused, "older helper"),
            (DialFault::Failed, "didn't start"),
            (DialFault::Spawn("no ssh".into()), "didn't start"),
        ];
        for (fault, word) in cases {
            let said = never_words(&host, &fault);
            assert!(said.contains(word) && said.contains("prod"), "{said}");
            assert!(!said.contains("ssh") && !said.contains("daemon"), "{said}");
        }
    }
}
