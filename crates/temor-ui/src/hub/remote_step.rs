//! Carrying out the helper's install and the first connection (PLAN §9b, Tier B): what a probe
//! found, the person's answer, the upload, the dial. `helper_step` decides; this runs what it
//! names. The connection's life after that is in `remote_link`.

use temor_core::remote::{CacheDir, Grant, HelperIn, HelperOut, HelperState, HostOs, helper_step};
use temor_core::vocab::ids::PaneId;
use temor_core::vocab::text::HelperHash;
use temor_ssh::HostAlias;
use temor_wire::digest::ContentHash;
use temor_wire::link::{Attempt, LinkState};

use super::model::Hub;
use super::remote::Busy;
use crate::edge::remote_builds::no_build_words;
use crate::edge::remote_event::{RemoteEvent, Wanted};
use crate::edge::remote_install::Found;

impl Hub {
    /// Carries out what a job or a connection reported, and returns the messages it left for the
    /// person.
    pub fn remote_event(&mut self, event: RemoteEvent) -> Vec<String> {
        match event {
            RemoteEvent::Probed { host, found } => self.remote_probed(&host, found),
            RemoteEvent::Installed { host, hash } => self.remote_installed(&host, hash),
            RemoteEvent::Dialed {
                host,
                generation,
                outcome,
            } => self.remote_dialed(&host, generation, outcome),
            RemoteEvent::Attached {
                host,
                generation,
                pane,
                session,
            } => self.remote_attached(&host, generation, pane, session),
            RemoteEvent::AttachFailed {
                host,
                generation,
                pane,
                fault,
            } => self.remote_attach_failed(&host, generation, pane, fault),
            RemoteEvent::Ports {
                host,
                generation,
                ports,
            } => self.remote_ports(&host, generation, ports),
            RemoteEvent::Exited {
                host,
                generation,
                session,
            } => self.remote_exited(&host, generation, session),
            RemoteEvent::Closed { host, generation } => self.remote_closed(&host, generation),
            RemoteEvent::Stalled { host, generation } => self.remote_stalled(&host, generation),
            RemoteEvent::Connected { pane } => self.remote_begin(pane),
        }
        std::mem::take(&mut self.notices)
    }

    fn remote_probed(&mut self, host: &HostAlias, found: Option<Found>) {
        let line = self.remote.lines.entry(host.clone()).or_default();
        line.busy = Busy::Idle;
        match found {
            Some(found) => {
                line.login = found.login;
                self.feed_helper(host, HelperIn::Probed(found.probe));
            }
            None => {
                line.helper = HelperState::Unsupported;
                self.remote_fallback_all(host);
            }
        }
    }

    fn remote_installed(&mut self, host: &HostAlias, hash: Option<HelperHash>) {
        let input = match hash {
            Some(hash) => HelperIn::Uploaded(hash),
            None => HelperIn::UploadFailed,
        };
        self.feed_helper(host, input);
    }

    /// The person answered the question a helper pane asks.
    pub fn remote_answer(&mut self, pane: PaneId, grant: Grant) {
        let Some(alias) = self.remote.panes.get(&pane).map(|p| p.alias.clone()) else {
            return;
        };
        let asking = self
            .remote
            .lines
            .get(&alias)
            .is_some_and(|line| line.helper == HelperState::Offered);
        if asking {
            self.feed_helper(&alias, HelperIn::Answered(grant));
        }
    }

    /// Moves `host`'s helper along and does what it asks.
    fn feed_helper(&mut self, host: &HostAlias, input: HelperIn) {
        if let HelperIn::Probed(probe) = &input {
            self.remote.lines.entry(host.clone()).or_default().arch = Some(probe.arch);
        }
        let arch = self.remote.lines.get(host).and_then(|line| line.arch);
        let Some(wanted) = arch
            .and_then(|arch| self.remote.builds.for_host(arch))
            .map(|blob| blob.name())
        else {
            return self.remote_without_build(host, &input);
        };
        let grant = self.helper_grant(host);
        let line = self.remote.lines.entry(host.clone()).or_default();
        let state = std::mem::replace(&mut line.helper, HelperState::Absent);
        let (state, outs) = helper_step(state, input, &wanted, grant);
        line.helper = state;
        for out in outs {
            match out {
                // The question shows in the pane while the helper stands at `Offered`.
                HelperOut::AskPerson => {}
                HelperOut::Remember(grant) => self.remember_helper(host, grant),
                HelperOut::Upload(_) => {
                    if let Some(blob) = arch
                        .and_then(|arch| self.remote.builds.for_host(arch))
                        .cloned()
                    {
                        self.remote.work.install(&self.ssh.env, host.clone(), blob);
                    }
                }
                HelperOut::RemoveFromHost(old) => {
                    if let Some(old) = ContentHash::from_hex(old.as_str()) {
                        self.remote.work.remove(&self.ssh.env, host.clone(), old);
                    }
                }
                HelperOut::UseHelper(_) => self.remote_dial_first(host),
                HelperOut::StayPlain => self.remote_fallback_all(host),
            }
        }
    }

    /// The host's kind of processor has no helper build here: it stays on a plain connection.
    /// A computer the helper could otherwise run on is told why; any other was never offered it.
    fn remote_without_build(&mut self, host: &HostAlias, input: &HelperIn) {
        if let Some(line) = self.remote.lines.get_mut(host) {
            line.helper = HelperState::Unsupported;
        }
        if let HelperIn::Probed(probe) = input
            && probe.os == HostOs::Linux
            && probe.cache == CacheDir::Usable
        {
            self.notices.push(no_build_words(host.as_str()));
        }
        self.remote_fallback_all(host);
    }

    /// Looks at a host with panes waiting, or opens their sessions when the connection is up.
    pub(super) fn remote_advance(&mut self, host: &HostAlias) {
        if !self.remote.builds.any() {
            return self.remote_fallback_all(host);
        }
        let ours = self.remote.builds.names();
        let line = self.remote.lines.entry(host.clone()).or_default();
        let up = line.conn.is_some() && matches!(line.link, LinkState::Connected { .. });
        if up {
            return self.remote_open_waiting(host);
        }
        if line.busy != Busy::Idle {
            return;
        }
        match line.helper {
            HelperState::Ready(_) => self.remote_dial_first(host),
            HelperState::Absent => {
                line.busy = Busy::Probing;
                self.remote.work.probe(&self.ssh.env, host.clone(), ours);
            }
            HelperState::Offered | HelperState::Uploading => {}
            HelperState::Refused | HelperState::Unsupported => self.remote_fallback_all(host),
        }
    }

    /// The first connection of the run to a host's helper.
    fn remote_dial_first(&mut self, host: &HostAlias) {
        let line = self.remote.lines.entry(host.clone()).or_default();
        line.link = LinkState::Reattaching {
            sessions: Vec::new(),
            attempt: Attempt(1),
        };
        self.remote_dial(host);
    }

    /// Starts a connection attempt for `host`: a new generation with a new router, so what the
    /// last connection says afterwards is ignored.
    pub(super) fn remote_dial(&mut self, host: &HostAlias) {
        if host.as_str() == super::remote::LOCAL {
            let Some(env) = self.local_dial_env() else {
                return self.local_fallback();
            };
            let line = self.remote.lines.entry(host.clone()).or_default();
            line.generation = line.generation.next();
            line.router = Default::default();
            line.busy = Busy::Dialing;
            let (generation, router) = (line.generation, line.router.clone());
            self.remote
                .work
                .dial_local(env, host.clone(), generation, router);
            return;
        }
        let arch = self.remote.lines.get(host).and_then(|line| line.arch);
        let Some(blob) = arch
            .and_then(|arch| self.remote.builds.for_host(arch))
            .cloned()
        else {
            return self.remote_fallback_all(host);
        };
        let line = self.remote.lines.entry(host.clone()).or_default();
        line.generation = line.generation.next();
        line.router = Default::default();
        line.busy = Busy::Dialing;
        let (generation, router) = (line.generation, line.router.clone());
        self.remote
            .work
            .dial(&self.ssh.env, host.clone(), generation, blob.hash, router);
    }

    /// Opens a session for each pane that waits on the connection.
    pub(super) fn remote_open_waiting(&mut self, host: &HostAlias) {
        let Some(line) = self.remote.lines.get_mut(host) else {
            return;
        };
        let Some(conn) = line.conn.clone() else {
            return;
        };
        let waiting = std::mem::take(&mut line.waiting);
        let env = line.login.session_env();
        let home = line.login.home().map(str::to_owned);
        let (generation, router) = (line.generation, line.router.clone());
        for pane in waiting {
            let Some(remote) = self.remote.panes.get(&pane) else {
                continue;
            };
            let (cwd, env) = match self.remote.local.get(&pane) {
                Some(own) => (own.cwd.clone(), own.env.clone()),
                None => (
                    self.ssh
                        .panes
                        .get(&pane)
                        .and_then(|host| host.start_dir.as_ref())
                        .map(|dir| dir.as_str().to_owned())
                        .or_else(|| home.clone())
                        .unwrap_or_else(|| "/".to_owned()),
                    env.clone(),
                ),
            };
            // A session the pane had when temor last quit is asked for before a new one.
            let wanted = match self.remote.saved.get(&pane) {
                Some(session) => Wanted::Resume {
                    session: *session,
                    cwd,
                    env,
                },
                None => Wanted::Fresh { cwd, env },
            };
            let ticket = crate::edge::remote_event::Ticket {
                pane,
                target: std::sync::Arc::clone(&remote.target),
                feeder: remote.feeder.clone(),
                wanted,
                palette: crate::edge::remote_palette::wire_palette(&self.palette),
            };
            self.remote.work.attach(
                std::sync::Arc::clone(&conn),
                router.clone(),
                (host.clone(), generation),
                ticket,
            );
        }
    }
}
