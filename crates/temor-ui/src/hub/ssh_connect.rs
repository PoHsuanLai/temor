//! Starting a session on a host (PLAN §9b "Opening"): the consent questions one at a time, then
//! the shared connection (the first pane makes it and asks for any password; the others join),
//! then `ssh` in the pane with the bootstrap or plain. The decisions are temor-core's `gate`,
//! `consent_step` and `master_step`; this carries out what they answer.

use std::cell::RefCell;
use std::rc::Rc;

use temor_client::model::host::consent::{ConsentIn, Gate, RunMode, Who, consent_step, gate};
use temor_client::model::host::master::MasterIn;
use temor_client::model::remote::Grant;
use temor_client::model::vocab::ids::PaneId;
use temor_client::model::workspace::{Spawn, WorkspaceIn};
use temor_client::ssh::{
    BootstrapOptions, BootstrapScript, ControlLayout, HostAlias, HostRow, RemoteFilePath, SshError,
    bootstrap_remote_command, plain_session_argv, session_argv,
};
use temor_panes::pty::ChildExit;

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
        // Declined means nothing of temor's reaches the host: no remote command, and no `cd`
        // typed after the prompt either, so a plain session starts in the host's own home folder
        // whatever folder the pane it came from was in.
        RunMode::Plain => Ok(plain_session_argv(alias, layout)),
        RunMode::Bootstrap(policy) => {
            let script = BootstrapScript::build_in(
                &temor_panes::shell::integration_scripts(),
                BootstrapOptions::with_terminfo(policy),
                start_dir,
            )?;
            let remote = bootstrap_remote_command(&script)?;
            Ok(session_argv(alias, layout, &remote))
        }
    }
}

/// What the person is told about a host name temor can't use. An IPv6 address can't be typed
/// here: it goes in the ssh config as the `HostName` of a short name.
pub(super) fn not_a_name(text: &str) -> String {
    match temor_client::ssh::is_ipv6_literal(text) {
        true => format!(
            "{text} is an IPv6 address; give it a short name with a HostName line in your \
             ssh config, then use that name"
        ),
        false => format!("{text} is not a name temor can connect to"),
    }
}

/// The IPv6 address at the front of a typed `address:/path`, if that is what it is: the person
/// meant a host, and gets the same word about the ssh config as for a host tab.
pub(super) fn ipv6_host_of(typed: &str) -> Option<&str> {
    let (host, _) = typed.trim().rsplit_once(":/")?;
    temor_client::ssh::is_ipv6_literal(host).then_some(host.trim())
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
        if row.online() == Some(false) {
            self.ssh.offline = Some(alias.clone());
        }
        self.open_host_tab(alias);
    }

    /// Opens a new tab that is a session on a host typed by name (`prod`, `me@prod`).
    pub fn open_host_named(&mut self, text: &str) {
        match HostAlias::new(text.trim()) {
            Ok(alias) => self.open_host_tab(alias),
            Err(_) => self.notices.push(not_a_name(text.trim())),
        }
    }

    fn open_host_tab(&mut self, alias: HostAlias) {
        self.ssh.intent = Some(alias);
        self.act(WorkspaceIn::NewTab { cwd: None });
        self.ssh.offline = None;
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
        let Some(alias) = self.alias_of(pane) else {
            return input;
        };
        self.ssh.intent = Some(alias);
        match input {
            WorkspaceIn::NewTab { .. } => WorkspaceIn::NewTab {
                cwd: self.book.ws.cwds.get(&pane).cloned(),
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
                let session = temor_client::remote::ids::RemoteSessionId(session.0);
                self.remote.saved.insert(spawn.pane, session);
            }
            self.ssh.deferred.insert(spawn.pane);
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
        let offline = self.ssh.offline.take().as_ref() == Some(&alias);
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
        if offline {
            // Say why before dialling a computer the tailnet cannot reach.
            if let Some(host) = self.ssh.panes.get_mut(&spawn.pane) {
                host.phase = Phase::Offline;
            }
            return;
        }
        self.gate_host(spawn.pane);
    }

    /// The person pressed "Try anyway" on a computer shown offline.
    pub fn try_offline(&mut self, pane: PaneId) {
        if self.ssh.panes.get(&pane).map(|host| host.phase) == Some(Phase::Offline) {
            self.gate_host(pane);
        }
    }

    pub(super) fn who_of(&self, alias: &HostAlias) -> Who {
        Who {
            key: self.ssh.lines.get(alias).and_then(|line| line.key.clone()),
            alias: alias.as_str().to_owned(),
        }
    }

    /// Asks the next open question, or starts the session when every one is answered.
    pub(super) fn gate_host(&mut self, pane: PaneId) {
        let Some(alias) = self.alias_of(pane) else {
            return;
        };
        let verdict = gate(&self.ssh.book, &self.who_of(&alias));
        let tier = self.remote_tier(&alias);
        let hidden = self.ssh.deferred.contains(&pane);
        let Some(host) = self.ssh.panes.get_mut(&pane) else {
            return;
        };
        match verdict {
            Gate::Ask(action) => host.phase = Phase::Asking(action),
            Gate::Run(mode) => {
                host.mode = Some(mode);
                host.tier = tier;
                host.phase = Phase::Waiting;
                // A restored pane connects when its tab is first in front (`ssh_start_shown`).
                if !hidden {
                    self.want_session(&alias, pane);
                }
            }
        }
    }

    /// Connects the restored panes whose tab is now in front.
    pub(super) fn ssh_start_shown(&mut self) {
        let front = self
            .book
            .ws
            .front()
            .and_then(|tab| self.book.ws.tabs.tab(tab))
            .map(|tab| tab.tree.panes())
            .unwrap_or_default();
        let shown: Vec<PaneId> = self
            .ssh
            .deferred
            .iter()
            .filter(|pane| front.contains(pane))
            .copied()
            .collect();
        for pane in shown {
            self.ssh.deferred.remove(&pane);
            self.gate_host(pane);
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
        let (book, changed) = consent_step(std::mem::take(&mut self.ssh.book), input);
        self.ssh.book = book;
        if changed {
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
            .book
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
        self.ssh.deferred.remove(&pane);
        if let Some(host) = self.ssh.panes.remove(&pane) {
            self.ssh_note_closed(pane, host.alias.clone());
            if matches!(host.phase, Phase::Connecting | Phase::Waiting) {
                self.feed_master(&host.alias, MasterIn::Failed(pane));
            }
            self.pace.changed();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ipv6_address_is_told_to_go_in_the_ssh_config_other_names_just_cannot_be_used() {
        let said = not_a_name("2001:db8::1");
        assert!(said.contains("HostName") && said.contains("IPv6"), "{said}");
        assert_eq!(not_a_name("a b"), "a b is not a name temor can connect to");
        let cases = [
            ("2001:db8::1:/etc/hosts", Some("2001:db8::1")),
            ("[::1]:/etc/hosts", Some("[::1]")),
            ("prod:/etc/hosts", None),
            ("/etc/hosts", None),
        ];
        for (typed, want) in cases {
            assert_eq!(ipv6_host_of(typed), want, "{typed}");
        }
    }
}
