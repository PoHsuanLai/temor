//! Starting a host pane on the helper (PLAN §9b, Tier B). The shared connection is still made
//! by an `ssh` in the pane, because it may ask for a password or a confirmation the person
//! answers there; that `ssh` runs `true` and leaves. Then the pane becomes a session on the
//! helper, or, when the host cannot or may not run it, an ordinary `ssh` session (Tier A).

use std::sync::{Arc, Mutex};

use temor_core::host::master::OpenAs;
use temor_core::notify::Focus;
use temor_core::remote::HelperState;
use temor_core::shelf::PaneSite;
use temor_core::vocab::ids::PaneId;
use temor_ssh::{ControlLayout, HostAlias};

use super::model::Hub;
use super::remote::{Busy, RemotePane, Shown, Tier};
use super::remote_retry::{Begin, begin_plan};
use super::remote_source::{Blob, Source, session_source};
use super::ssh::{Check, Phase};
use super::ssh_check::RawCheck;
use crate::edge::remote_link::{RemoteLink, Target, lock};
use crate::edge::remote_relay::relay;
use crate::view::launch::{Program, open_session_with};
use crate::view::remote_launch::open_fed_session;

/// The `ssh` arguments that make the shared connection and leave: a terminal for the questions
/// it may ask, the connection kept in the background for the helper's `ssh` to join, and `true`
/// as the only thing run.
pub fn connect_argv(alias: &HostAlias, layout: &ControlLayout) -> Vec<String> {
    let mut argv = vec!["-t".to_owned()];
    for option in [
        "ControlMaster=auto".to_owned(),
        layout.control_path_option(),
        layout.control_persist_option(),
    ] {
        argv.push("-o".to_owned());
        argv.push(option);
    }
    argv.extend([
        "--".to_owned(),
        alias.as_str().to_owned(),
        "true".to_owned(),
    ]);
    argv
}

impl Hub {
    /// What a session on `alias` runs on, given what the person answered and what is known.
    pub(super) fn remote_tier(&self, alias: &HostAlias) -> Tier {
        let blob = match self.remote.builds.any() {
            true => Blob::Have,
            false => Blob::Missing,
        };
        let grant = self.helper_grant(alias);
        let absent = HelperState::Absent;
        let helper = self
            .remote
            .lines
            .get(alias)
            .map_or(&absent, |line| &line.helper);
        match session_source(grant, blob, helper) {
            Source::Plain => Tier::Plain,
            Source::Helper => Tier::Helper,
        }
    }

    /// The shared connection's turn came for a helper pane: `First` makes the connection in the
    /// pane, `Join` finds it up.
    pub(super) fn start_helper(&mut self, alias: &HostAlias, pane: PaneId, how: OpenAs) {
        match how {
            OpenAs::Join => self.remote_begin(pane),
            OpenAs::First => self.start_connect(alias, pane),
        }
    }

    /// Runs the `ssh` that makes the shared connection in the pane.
    pub(super) fn start_connect(&mut self, alias: &HostAlias, pane: PaneId) {
        let Ok(layout) = self.ssh.env.layout.clone() else {
            return;
        };
        let scan = Arc::new(RawCheck::new());
        let program = Program {
            program: self.ssh.env.tools.ssh.clone(),
            args: connect_argv(alias, &layout)
                .into_iter()
                .map(Into::into)
                .collect(),
            tap: Some(scan.listener()),
        };
        let start = self.space_start();
        let events = relay(self.events.clone(), self.remote.work.sender());
        let opened = open_session_with(
            &self.launch,
            self.palette,
            (pane, None, None),
            events,
            &start.env,
            None,
            Some(program),
        );
        match opened {
            Ok(mut session) => {
                session.set_focus(self.focus_of(pane));
                // A pane on a host files for another computer: its paths mean nothing here.
                session.set_site(PaneSite::Remote);
                if let Some(slot) = self.slots.get(&pane) {
                    slot.borrow_mut().session = Some(session);
                }
                if let Some(host) = self.ssh.panes.get_mut(&pane) {
                    host.phase = Phase::Connecting;
                    host.scan = Some(scan);
                }
            }
            Err(err) => {
                self.feed_master(alias, temor_core::host::master::MasterIn::Failed(pane));
                self.fail_host(pane, err.to_string());
            }
        }
    }

    fn focus_of(&self, pane: PaneId) -> Focus {
        match self.keyboard_pane() == Some(pane) {
            true => Focus::Focused,
            false => Focus::Unfocused,
        }
    }

    /// The shared connection is up: the pane becomes a session on the helper.
    pub(super) fn remote_begin(&mut self, pane: PaneId) {
        let Some(host) = self.ssh.panes.get_mut(&pane) else {
            return;
        };
        host.phase = Phase::Up;
        host.scan = None;
        host.check = Check::Nothing;
        let alias = host.alias.clone();
        // A pane that had a session when the link was lost asks for the same one.
        let before = self.remote.panes.get(&pane).map(|old| {
            let target = lock(&old.target);
            (old.session, old.shown, target.size, target.cell)
        });
        let begin = begin_plan(before.map(|(session, shown, ..)| (session, shown)));
        self.remote_drop_pane(pane);
        let mut start = Target::default();
        if let Some((.., size, cell)) = before {
            (start.size, start.cell) = (size, cell);
        }
        let target = Arc::new(Mutex::new(start));
        let link = match RemoteLink::start(Arc::clone(&target)) {
            Ok(link) => link,
            Err(err) => return self.fail_host(pane, err.to_string()),
        };
        let opened = open_fed_session(&self.launch, self.palette, pane, self.events.clone(), link);
        let (mut session, feeder) = match opened {
            Ok(opened) => opened,
            Err(err) => return self.fail_host(pane, err.to_string()),
        };
        session.set_focus(self.focus_of(pane));
        if let Some(slot) = self.slots.get(&pane) {
            slot.borrow_mut().session = Some(session);
        }
        self.remote.panes.insert(
            pane,
            RemotePane {
                alias: alias.clone(),
                target,
                feeder,
                session: match begin {
                    Begin::Fresh => None,
                    Begin::Rejoin(session) => Some(session),
                },
                shown: Shown::Opening,
            },
        );
        match begin {
            Begin::Fresh => {
                self.remote
                    .lines
                    .entry(alias.clone())
                    .or_default()
                    .waiting
                    .push(pane);
                self.remote_advance(&alias);
            }
            Begin::Rejoin(_) => self.remote_rejoin(&alias),
        }
    }

    /// Forgets what a pane held on a helper (a restarted pane starts over).
    fn remote_drop_pane(&mut self, pane: PaneId) {
        let Some(old) = self.remote.panes.remove(&pane) else {
            return;
        };
        if let Some(line) = self.remote.lines.get_mut(&old.alias) {
            line.waiting.retain(|waiting| *waiting != pane);
            if let Some(session) = old.session {
                line.router.forget(session);
            }
        }
    }

    /// The pane gives up the helper and becomes a plain `ssh` session.
    pub(super) fn remote_fallback(&mut self, pane: PaneId) {
        let Some(alias) = self.ssh.panes.get(&pane).map(|host| host.alias.clone()) else {
            return;
        };
        self.remote_drop_pane(pane);
        self.remote.saved.remove(&pane);
        if let Some(host) = self.ssh.panes.get_mut(&pane) {
            host.tier = Tier::Plain;
        }
        self.start_session(&alias, pane, OpenAs::Join);
    }

    /// Every pane of `alias` still waiting on the helper becomes a plain `ssh` session.
    pub(super) fn remote_fallback_all(&mut self, alias: &HostAlias) {
        if let Some(line) = self.remote.lines.get_mut(alias) {
            line.busy = Busy::Idle;
        }
        let waiting: Vec<PaneId> = self
            .remote
            .panes
            .iter()
            .filter(|(_, pane)| pane.alias == *alias && pane.shown == Shown::Opening)
            .map(|(id, _)| *id)
            .collect();
        for pane in waiting {
            self.remote_fallback(pane);
        }
    }

    /// A host pane was closed: its session on the host ends.
    pub(super) fn remote_forget(&mut self, pane: PaneId) {
        self.remote.saved.remove(&pane);
        let Some(old) = self.remote.panes.remove(&pane) else {
            return;
        };
        let Some(line) = self.remote.lines.get_mut(&old.alias) else {
            return;
        };
        line.waiting.retain(|waiting| *waiting != pane);
        if let Some(session) = old.session {
            line.router.forget(session);
            if let Some(conn) = line.conn.clone() {
                self.remote.work.close(conn, session);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use temor_ssh::{ControlPersist, RuntimeDir, Seconds};

    #[test]
    fn the_connecting_ssh_has_a_terminal_leaves_the_connection_behind_and_runs_only_true() {
        let runtime = RuntimeDir::new(std::path::Path::new("/run/user/1000/temor"))
            .expect("an absolute folder");
        let persist = ControlPersist::Idle(Seconds::new(600).expect("positive"));
        let layout = ControlLayout::new(runtime, persist).expect("a short path");
        let argv = connect_argv(&HostAlias::new("prod").expect("a name"), &layout);
        assert_eq!(argv.first().map(String::as_str), Some("-t"));
        assert!(argv.iter().any(|a| a == "ControlPersist=600"));
        let tail: Vec<&str> = argv.iter().rev().take(3).map(String::as_str).collect();
        assert_eq!(tail, ["true", "prod", "--"]);
    }
}
