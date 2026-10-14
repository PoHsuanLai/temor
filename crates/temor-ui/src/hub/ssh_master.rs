//! The shared connection of a host (PLAN §9b "Opening"): the first pane makes it, the others
//! join, and the host's key is read once it is up. The decisions are temor-core's `master_step`;
//! this runs the probe and starts the `ssh` it names.

use std::sync::Arc;

use temor_core::host::consent::{ConsentIn, Who};
use temor_core::host::master::{Master, MasterIn, MasterOut, OpenAs, master_step};
use temor_core::notify::Focus;
use temor_core::shelf::PaneSite;
use temor_core::vocab::ids::PaneId;
use temor_ssh::{
    Fingerprint, HostAlias, HostKey, HostName, KnownName, MasterProbe, prepare_control_dirs,
    socket_appeared,
};

use super::model::Hub;
use super::remote::Tier;
use super::ssh::{Check, Phase};
use super::ssh_check::RawCheck;
use super::ssh_connect::session_args;
use crate::view::launch::{Program, open_session_with};

impl Hub {
    /// Asks the shared connection for a session for `pane`.
    pub(super) fn want_session(&mut self, alias: &HostAlias, pane: PaneId) {
        let layout = match &self.ssh.env.layout {
            Ok(layout) => layout.clone(),
            Err(why) => {
                let why = format!("temor can't share connections here: {why}");
                return self.fail_host(pane, why);
            }
        };
        if let Err(err) = prepare_control_dirs(&layout) {
            return self.fail_host(pane, err.to_string());
        }
        // A connection nobody uses may have timed out since: ask the system again.
        let busy = self.ssh.panes.iter().any(|(other, host)| {
            host.alias == *alias
                && *other != pane
                && !matches!(host.phase, Phase::Failed | Phase::Offline)
        });
        let line = self.ssh.lines.entry(alias.clone()).or_default();
        if line.master == Master::Up && !busy {
            line.master = Master::Down;
        }
        self.feed_master(alias, MasterIn::Wanted(pane));
    }

    pub(super) fn feed_master(&mut self, alias: &HostAlias, input: MasterIn) {
        let line = self.ssh.lines.entry(alias.clone()).or_default();
        let (master, outs) = master_step(std::mem::take(&mut line.master), input);
        line.master = master;
        for out in outs {
            match out {
                MasterOut::Probe => self.ssh.work.probe(alias.clone()),
                MasterOut::Open { pane, how } => {
                    if how == OpenAs::First {
                        self.note_sockets(alias);
                    }
                    self.start_session(alias, pane, how)
                }
                MasterOut::ConnectionEnded => {
                    self.feed_forwards(alias, temor_core::host::forward::ForwardIn::ConnectionLost)
                }
            }
        }
    }

    /// Remembers which connections exist before `alias`'s is made.
    fn note_sockets(&mut self, alias: &HostAlias) {
        if let Ok(layout) = &self.ssh.env.layout {
            let before = layout.finished_sockets();
            self.ssh.baselines.insert(alias.clone(), before);
        }
    }

    /// Whether the one connection being made has its socket. With several being made at once a
    /// new socket cannot be told apart, so each is asked after instead.
    pub(super) fn socket_up(&self, alias: &HostAlias) -> bool {
        let (Ok(layout), Some(before)) = (&self.ssh.env.layout, self.ssh.baselines.get(alias))
        else {
            return false;
        };
        socket_appeared(before, &layout.finished_sockets())
    }

    /// Starts `ssh` in `pane`, to make the connection (`First`) or join it (`Join`).
    pub(super) fn start_session(&mut self, alias: &HostAlias, pane: PaneId, how: OpenAs) {
        let Some(host) = self.ssh.panes.get(&pane) else {
            return;
        };
        if host.tier == Tier::Helper {
            return self.start_helper(alias, pane, how);
        }
        let (mode, dir) = (host.mode, host.start_dir.clone());
        let (Some(mode), Ok(layout)) = (mode, self.ssh.env.layout.clone()) else {
            return;
        };
        let args = match session_args(mode, alias, &layout, dir.as_ref()) {
            Ok(args) => args,
            Err(err) => {
                self.feed_master(alias, MasterIn::Failed(pane));
                return self.fail_host(pane, err.to_string());
            }
        };
        // Only the session that makes the connection can be asked for the browser check.
        let scan = match how {
            OpenAs::First => Some(Arc::new(RawCheck::new())),
            OpenAs::Join => None,
        };
        let program = Program {
            program: self.ssh.env.tools.ssh.clone(),
            args: args.into_iter().map(Into::into).collect(),
            tap: scan.as_ref().map(RawCheck::listener),
        };
        let start = self.space_start();
        let opened = open_session_with(
            &self.launch,
            self.palette,
            (pane, None, None),
            self.events.clone(),
            &start.env,
            None,
            Some(program),
        );
        match opened {
            Ok(mut session) => {
                session.set_focus(match self.keyboard_pane() == Some(pane) {
                    true => Focus::Focused,
                    false => Focus::Unfocused,
                });
                // A pane on a host files for another computer: its paths mean nothing here.
                session.set_site(PaneSite::Remote);
                if let Some(slot) = self.slots.get(&pane) {
                    slot.borrow_mut().session = Some(session);
                }
                if let Some(host) = self.ssh.panes.get_mut(&pane) {
                    host.phase = match how {
                        OpenAs::First => Phase::Connecting,
                        OpenAs::Join => Phase::Up,
                    };
                    host.scan = scan;
                }
            }
            Err(err) => {
                self.feed_master(alias, MasterIn::Failed(pane));
                self.fail_host(pane, err.to_string());
            }
        }
    }

    /// Hears whether a host's connection is running.
    pub(super) fn on_probed(&mut self, alias: HostAlias, probe: MasterProbe) {
        let master = self.ssh.lines.get(&alias).map(|line| line.master.clone());
        match master {
            Some(Master::Probing { .. }) => self.feed_master(&alias, MasterIn::Probed(probe)),
            Some(Master::Connecting { .. }) if probe == MasterProbe::Running => {
                self.established(&alias)
            }
            _ => {}
        }
    }

    /// The first session of a host is connected: the host's key was verified.
    pub(super) fn established(&mut self, alias: &HostAlias) {
        for pane in self.ssh.panes_on(alias) {
            if let Some(host) = self.ssh.panes.get_mut(&pane) {
                if host.phase == Phase::Connecting {
                    host.phase = Phase::Up;
                }
                host.scan = None;
                host.check = Check::Nothing;
            }
        }
        self.feed_master(alias, MasterIn::Established);
        let list = self.ssh.env.host_list();
        let (name, port) = match list.entries.iter().find(|entry| &entry.alias == alias) {
            Some(entry) => (Some(entry.hostname.clone()), entry.port),
            None => {
                let bare = alias.as_str().rsplit('@').next().unwrap_or_default();
                (HostName::new(bare), None)
            }
        };
        let fallback = name.map(|host| KnownName { host, port });
        self.ssh.work.identify(alias.clone(), fallback);
    }

    /// Hears the fingerprint of the key a host's connection verified: answers recorded under
    /// the alias now belong to the machine.
    pub(super) fn on_identified(&mut self, alias: HostAlias, fingerprint: Option<Fingerprint>) {
        let line = self.ssh.lines.entry(alias.clone()).or_default();
        let node = line
            .key
            .as_ref()
            .and_then(|key| key.node().map(|node| node.as_str().to_owned()));
        let key = HostKey::from_parts(
            fingerprint.as_ref().map(Fingerprint::as_str),
            node.as_deref(),
        )
        .or_else(|| line.key.clone());
        line.key = key.clone();
        if let Some(key) = key {
            let who = Who {
                key: Some(key),
                alias: alias.as_str().to_owned(),
            };
            self.step_book(ConsentIn::Identified { who });
        }
    }
}
