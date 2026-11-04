//! A host pane's own step (PLAN §4c, "machines are steps"): what happens to its phase, its
//! startup, its browser check and its tier, and what the hub is asked to do about it. The hub
//! turns what it hears into a [`HostIn`] and carries out the [`HostOut`]s ([`Hub::host_in`]).

use std::sync::Arc;

use temor_client::model::host::consent::{BootstrapAction, RunMode};
use temor_client::model::host::master::MasterIn;
use temor_client::model::vocab::ids::PaneId;
use temor_client::ssh::{CheckUrl, RemoteFilePath};
use temor_panes::pty::ChildExit;

use super::model::Hub;
use super::remote::Tier;
use super::ssh::{Check, HostPane, Phase};
use super::ssh_check::RawCheck;

/// What happens to a host pane.
pub(super) enum HostIn {
    /// The tailnet says the computer is offline.
    Offline,
    /// The person pressed "Try anyway" on a computer shown offline.
    TryAnyway,
    /// Its tab came to the front: a restored pane waits no longer.
    Shown,
    /// A question about the setup is open.
    Asked(BootstrapAction),
    /// Every question is answered: how the session starts, and on what.
    Cleared { mode: RunMode, tier: Tier },
    /// Its `ssh` started, in `phase`, reading what it prints for the check when `scan` is set.
    Opened {
        phase: Phase,
        scan: Option<Arc<RawCheck>>,
    },
    /// The host's first session connected.
    Established,
    /// Its session on the helper opened.
    HelperOpened,
    /// It gives up the helper and becomes a plain `ssh` session.
    Plain,
    /// What `ssh` printed holds a browser check.
    CheckFound(CheckUrl),
    /// Its shell started: any check is over.
    ShellStarted,
    /// Its session could not start.
    Failed,
    /// Its program ended.
    Ended(Option<ChildExit>),
    /// It starts again, in `dir` on the host.
    Restart { dir: Option<RemoteFilePath> },
    /// It closed.
    Closed,
}

/// What a host pane asks of the hub.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HostOut {
    /// Ask the open questions again, or start the session.
    Gate,
    /// Ask the shared connection for a session.
    Want,
    /// Tell the shared connection this pane's session will not come up.
    MasterFailed,
    /// Tell the shared connection it is gone.
    MasterLost,
    /// Mark its tab, unless it is in front.
    Attention,
}

impl HostPane {
    /// The pane after `input`, and what the hub is asked to do.
    pub(super) fn step(&mut self, input: HostIn) -> Vec<HostOut> {
        match input {
            HostIn::Offline => {
                self.phase = Phase::Offline;
                vec![]
            }
            HostIn::TryAnyway => match self.phase {
                Phase::Offline => vec![HostOut::Gate],
                _ => vec![],
            },
            HostIn::Shown => match std::mem::replace(&mut self.deferred, false) {
                true => vec![HostOut::Gate],
                false => vec![],
            },
            HostIn::Asked(action) => {
                self.phase = Phase::Asking(action);
                vec![]
            }
            HostIn::Cleared { mode, tier } => {
                self.mode = Some(mode);
                self.tier = tier;
                self.phase = Phase::Waiting;
                // A restored pane connects when its tab is first in front.
                match self.deferred {
                    true => vec![],
                    false => vec![HostOut::Want],
                }
            }
            HostIn::Opened { phase, scan } => {
                self.phase = phase;
                self.scan = scan;
                vec![]
            }
            HostIn::Established => {
                if self.phase == Phase::Connecting {
                    self.phase = Phase::Up;
                }
                self.check_over();
                vec![]
            }
            HostIn::HelperOpened => {
                self.phase = Phase::Up;
                self.check_over();
                vec![]
            }
            HostIn::Plain => {
                self.tier = Tier::Plain;
                vec![]
            }
            HostIn::CheckFound(url) => {
                let new = self.check == Check::Nothing;
                self.check = Check::Shown(url);
                match new {
                    true => vec![HostOut::Attention],
                    false => vec![],
                }
            }
            HostIn::ShellStarted => {
                self.check_over();
                vec![]
            }
            HostIn::Failed => {
                self.phase = Phase::Failed;
                vec![]
            }
            HostIn::Ended(exit) => {
                let was = std::mem::replace(&mut self.phase, Phase::Failed);
                self.check_over();
                match (was, exit) {
                    (Phase::Connecting | Phase::Waiting, _) => vec![HostOut::MasterFailed],
                    // ssh itself failed on a connection that was up: the master is gone. (A
                    // helper session's own exit code says nothing of the connection.)
                    (Phase::Up, Some(ChildExit::Exited(255))) if self.tier == Tier::Plain => {
                        vec![HostOut::MasterLost]
                    }
                    _ => vec![],
                }
            }
            HostIn::Restart { dir } => {
                self.phase = Phase::Waiting;
                self.start_dir = dir;
                vec![HostOut::Gate]
            }
            HostIn::Closed => match self.phase {
                Phase::Connecting | Phase::Waiting => vec![HostOut::MasterFailed],
                _ => vec![],
            },
        }
    }

    fn check_over(&mut self) {
        self.scan = None;
        self.check = Check::Nothing;
    }
}

impl Hub {
    /// Steps `pane`'s host state with `input` and carries out what it asks. False when the pane
    /// is not a session on a host.
    pub(super) fn host_in(&mut self, pane: PaneId, input: HostIn) -> bool {
        let Some(host) = self.panes.host_mut(pane) else {
            return false;
        };
        let outs = host.step(input);
        let alias = host.alias.clone();
        self.host_outs(pane, &alias, outs);
        true
    }

    /// Carries out what `pane`, a session on `alias`, asked.
    pub(super) fn host_outs(
        &mut self,
        pane: PaneId,
        alias: &temor_client::ssh::HostAlias,
        outs: Vec<HostOut>,
    ) {
        for out in outs {
            match out {
                HostOut::Gate => self.gate_host(pane),
                HostOut::Want => self.want_session(alias, pane),
                HostOut::MasterFailed => self.feed_master(alias, MasterIn::Failed(pane)),
                HostOut::MasterLost => self.feed_master(alias, MasterIn::Lost),
                HostOut::Attention => self.host_attention(vec![pane]),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use temor_client::ssh::HostAlias;

    fn pane(phase: Phase, tier: Tier, deferred: bool) -> HostPane {
        HostPane {
            alias: HostAlias::new("box").expect("an alias"),
            phase,
            mode: None,
            start_dir: None,
            scan: None,
            check: Check::Nothing,
            tier,
            deferred,
        }
    }

    fn url() -> CheckUrl {
        CheckUrl::new("https://login.tailscale.com/a/check").expect("a check url")
    }

    #[test]
    fn each_input_moves_the_phase_and_asks_what_the_table_says() {
        use HostOut as O;
        use Phase as P;
        let exited = |code| HostIn::Ended(Some(ChildExit::Exited(code)));
        // before (phase, tier, deferred), input, after phase, outputs
        type Row = (P, Tier, bool, HostIn, P, Vec<O>);
        let table: Vec<Row> = vec![
            (
                P::Waiting,
                Tier::Plain,
                false,
                HostIn::Offline,
                P::Offline,
                vec![],
            ),
            (
                P::Offline,
                Tier::Plain,
                false,
                HostIn::TryAnyway,
                P::Offline,
                vec![O::Gate],
            ),
            (P::Up, Tier::Plain, false, HostIn::TryAnyway, P::Up, vec![]),
            (
                P::Waiting,
                Tier::Plain,
                true,
                HostIn::Shown,
                P::Waiting,
                vec![O::Gate],
            ),
            (
                P::Waiting,
                Tier::Plain,
                false,
                HostIn::Shown,
                P::Waiting,
                vec![],
            ),
            (
                P::Waiting,
                Tier::Plain,
                false,
                HostIn::Cleared {
                    mode: RunMode::Plain,
                    tier: Tier::Helper,
                },
                P::Waiting,
                vec![O::Want],
            ),
            (
                P::Waiting,
                Tier::Plain,
                true,
                HostIn::Cleared {
                    mode: RunMode::Plain,
                    tier: Tier::Plain,
                },
                P::Waiting,
                vec![],
            ),
            (
                P::Connecting,
                Tier::Plain,
                false,
                HostIn::Established,
                P::Up,
                vec![],
            ),
            (
                P::Failed,
                Tier::Plain,
                false,
                HostIn::Established,
                P::Failed,
                vec![],
            ),
            (
                P::Connecting,
                Tier::Helper,
                false,
                HostIn::HelperOpened,
                P::Up,
                vec![],
            ),
            (
                P::Connecting,
                Tier::Plain,
                false,
                HostIn::Ended(None),
                P::Failed,
                vec![O::MasterFailed],
            ),
            (
                P::Waiting,
                Tier::Plain,
                false,
                exited(1),
                P::Failed,
                vec![O::MasterFailed],
            ),
            (
                P::Up,
                Tier::Plain,
                false,
                exited(255),
                P::Failed,
                vec![O::MasterLost],
            ),
            (P::Up, Tier::Helper, false, exited(255), P::Failed, vec![]),
            (P::Up, Tier::Plain, false, exited(0), P::Failed, vec![]),
            (
                P::Failed,
                Tier::Plain,
                false,
                HostIn::Restart { dir: None },
                P::Waiting,
                vec![O::Gate],
            ),
            (
                P::Connecting,
                Tier::Plain,
                false,
                HostIn::Closed,
                P::Connecting,
                vec![O::MasterFailed],
            ),
            (P::Up, Tier::Plain, false, HostIn::Closed, P::Up, vec![]),
            (
                P::Waiting,
                Tier::Plain,
                false,
                HostIn::Failed,
                P::Failed,
                vec![],
            ),
        ];
        for (before, tier, deferred, input, after, want) in table {
            let mut host = pane(before, tier, deferred);
            let outs = host.step(input);
            assert_eq!((host.phase, outs), (after, want), "from {before:?}");
        }
    }

    #[test]
    fn a_check_is_shown_once_and_ends_when_the_shell_starts() {
        let mut host = pane(Phase::Connecting, Tier::Plain, false);
        assert_eq!(
            host.step(HostIn::CheckFound(url())),
            vec![HostOut::Attention]
        );
        assert_eq!(host.step(HostIn::CheckFound(url())), vec![]);
        assert_eq!(host.check, Check::Shown(url()));
        assert_eq!(host.step(HostIn::ShellStarted), vec![]);
        assert_eq!(host.check, Check::Nothing);
    }
}
