//! Starting a session on a host (PLAN §9b "Opening"): the consent questions one at a time, then
//! the shared connection (the first pane makes it and asks for any password; the others join),
//! then `ssh` in the pane with the bootstrap or plain. The decisions are temor-core's `gate`,
//! `consent_step` and `master_step`; this carries out what they answer.

use std::cell::RefCell;
use std::rc::Rc;

use temor_core::host::consent::{ConsentIn, ConsentOut, Gate, RunMode, Who, consent_step, gate};
use temor_core::host::master::MasterIn;
use temor_core::remote::Grant;
use temor_core::vocab::ids::PaneId;
use temor_core::workspace::{Spawn, WorkspaceIn};
use temor_pty::ChildExit;
use temor_ssh::{
    BootstrapOptions, BootstrapScript, ControlLayout, HostAlias, HostRow, RemoteFilePath, SshError,
    bootstrap_remote_command, plain_session_argv, session_argv,
};

use super::model::Hub;
use super::remote::Tier;
use super::ssh::{Check, HostPane, Phase};
use crate::view::slot::Slot;

/// The arguments of the host session: plain `ssh`, or `ssh` running the bootstrap that opens in
/// `start_dir` when it is given.
pub(super) fn session_args(
    mode: RunMode,
    alias: &HostAlias,
    layout: &ControlLayout,
    start_dir: Option<&RemoteFilePath>,
) -> Result<Vec<String>, SshError> {
    match mode {
        RunMode::Plain => Ok(plain_session_argv(alias, layout)),
        RunMode::Bootstrap(policy) => {
            let script = BootstrapScript::build_in(
                &temor_shell::integration_scripts(),
                BootstrapOptions::with_terminfo(policy),
                start_dir,
            )?;
            let remote = bootstrap_remote_command(&script)?;
            Ok(session_argv(alias, layout, &remote))
        }
    }
}

impl Hub {
    /// Opens a new tab that is a session on the host `row` names.
    pub fn open_host(&mut self, row: &HostRow) {
        let Some(alias) = row.destination() else {
            let name = row.shown();
            self.notices
                .push(format!("{name} has no name temor can connect to"));
            return;
        };
        let line = self.ssh.lines.entry(alias.clone()).or_default();
        if let Some(key) = row.key() {
            line.key = Some(key);
        }
        line.direct = row.direct_host();
        self.open_host_tab(alias);
    }

    /// Opens a new tab that is a session on a host typed by name (`prod`, `me@prod`).
    pub fn open_host_named(&mut self, text: &str) {
        match HostAlias::new(text.trim()) {
            Ok(alias) => self.open_host_tab(alias),
            Err(_) => self.notices.push(format!(
                "{} is not a name temor can connect to",
                text.trim()
            )),
        }
    }

    fn open_host_tab(&mut self, alias: HostAlias) {
        self.ssh.intent = Some(alias);
        self.act(WorkspaceIn::NewTab { cwd: None });
    }

    /// A split or a new tab of a host pane is a session on the same host, in the pane's folder.
    pub(super) fn ssh_prepare(&mut self, input: WorkspaceIn) -> WorkspaceIn {
        let inherits = matches!(
            input,
            WorkspaceIn::Split { .. } | WorkspaceIn::NewTab { cwd: None }
        );
        if self.ssh.intent.is_some() || !inherits {
            return input;
        }
        let Some(pane) = self.keyboard_pane() else {
            return input;
        };
        let Some(alias) = self.ssh.panes.get(&pane).map(|host| host.alias.clone()) else {
            return input;
        };
        self.ssh.intent = Some(alias);
        match input {
            WorkspaceIn::NewTab { .. } => WorkspaceIn::NewTab {
                cwd: self.ws.cwds.get(&pane).cloned(),
            },
            other => other,
        }
    }

    /// The host a pane about to start is a session on: the one just asked for, or the one it was
    /// on when the session was saved.
    pub(super) fn ssh_claims(&mut self, spawn: &Spawn) -> Option<HostAlias> {
        if let Some(at) = self.ssh.restored.iter().position(|r| r.pane == spawn.pane) {
            let saved = self.ssh.restored.remove(at);
            if let Some(session) = saved.session {
                let session = temor_wire::ids::SessionId(session.0);
                self.remote.saved.insert(spawn.pane, session);
            }
            return HostAlias::new(&saved.alias).ok();
        }
        self.ssh_reclaim(spawn.pane)
            .or_else(|| self.ssh.intent.take())
    }

    /// Makes `spawn`'s pane a session on `alias`: an empty pane now, `ssh` in it once the
    /// questions are answered and the connection's turn comes.
    pub(super) fn begin_host(&mut self, spawn: &Spawn, alias: HostAlias) {
        let slot = Slot {
            palette: self.palette,
            ..Slot::default()
        };
        self.slots.insert(spawn.pane, Rc::new(RefCell::new(slot)));
        let start_dir = spawn
            .cwd
            .as_ref()
            .and_then(|cwd| RemoteFilePath::new(cwd.as_str()).ok());
        self.ssh.panes.insert(
            spawn.pane,
            HostPane {
                alias,
                phase: Phase::Waiting,
                mode: None,
                start_dir,
                scan: None,
                check: Check::Nothing,
                tier: Tier::Plain,
            },
        );
        self.pace.changed();
        self.gate_host(spawn.pane);
    }

    pub(super) fn who_of(&self, alias: &HostAlias) -> Who {
        Who {
            key: self.ssh.lines.get(alias).and_then(|line| line.key.clone()),
            alias: alias.as_str().to_owned(),
        }
    }

    /// Asks the next open question, or starts the session when every one is answered.
    fn gate_host(&mut self, pane: PaneId) {
        let Some(alias) = self.ssh.panes.get(&pane).map(|host| host.alias.clone()) else {
            return;
        };
        let verdict = gate(&self.ssh.book, &self.who_of(&alias));
        let tier = self.remote_tier(&alias);
        let Some(host) = self.ssh.panes.get_mut(&pane) else {
            return;
        };
        match verdict {
            Gate::Ask(action) => host.phase = Phase::Asking(action),
            Gate::Run(mode) => {
                host.mode = Some(mode);
                host.tier = tier;
                host.phase = Phase::Waiting;
                self.want_session(&alias, pane);
            }
        }
    }

    /// The person answered the question a host pane asks.
    pub fn answer_host(&mut self, pane: PaneId, grant: Grant) {
        let Some((alias, Phase::Asking(action))) = self
            .ssh
            .panes
            .get(&pane)
            .map(|host| (host.alias.clone(), host.phase))
        else {
            return;
        };
        let who = self.who_of(&alias);
        self.step_book(ConsentIn::Answered { who, action, grant });
        self.gate_host(pane);
    }

    pub(super) fn step_book(&mut self, input: ConsentIn) {
        let (book, outs) = consent_step(std::mem::take(&mut self.ssh.book), input);
        self.ssh.book = book;
        if outs.contains(&ConsentOut::Persist) {
            self.pace.changed();
        }
    }

    pub(super) fn fail_host(&mut self, pane: PaneId, why: String) {
        self.notices.push(why);
        if let Some(host) = self.ssh.panes.get_mut(&pane) {
            host.phase = Phase::Failed;
        }
    }

    /// A host pane's program ended.
    pub(super) fn ssh_ended(&mut self, pane: PaneId, exit: Option<ChildExit>) {
        let Some(host) = self.ssh.panes.get_mut(&pane) else {
            return;
        };
        let (alias, was, tier) = (host.alias.clone(), host.phase, host.tier);
        host.phase = Phase::Failed;
        host.scan = None;
        host.check = Check::Nothing;
        match (was, exit) {
            (Phase::Connecting | Phase::Waiting, _) => {
                self.feed_master(&alias, MasterIn::Failed(pane))
            }
            // ssh itself failed on a connection that was up: the master is gone.
            // (A helper session's own exit code says nothing of the connection.)
            (Phase::Up, Some(ChildExit::Exited(255))) if tier == Tier::Plain => {
                self.feed_master(&alias, MasterIn::Lost)
            }
            _ => {}
        }
    }

    /// Starts a host pane's session again (its program ended), in the folder it was last in.
    pub(super) fn ssh_restart(&mut self, pane: PaneId) -> bool {
        let dir = self
            .ws
            .cwds
            .get(&pane)
            .and_then(|cwd| RemoteFilePath::new(cwd.as_str()).ok());
        let Some(host) = self.ssh.panes.get_mut(&pane) else {
            return false;
        };
        host.phase = Phase::Waiting;
        host.start_dir = dir;
        self.gate_host(pane);
        true
    }

    /// A host pane was closed.
    pub(super) fn ssh_forget(&mut self, pane: PaneId) {
        self.remote_forget(pane);
        if let Some(host) = self.ssh.panes.remove(&pane) {
            self.ssh_note_closed(pane, host.alias.clone());
            if matches!(host.phase, Phase::Connecting | Phase::Waiting) {
                self.feed_master(&host.alias, MasterIn::Failed(pane));
            }
            self.pace.changed();
        }
    }
}
