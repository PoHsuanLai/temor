//! The shared connection (ControlMaster) of a host: making its socket directory and asking
//! whether its master is running. The session argv that starts or joins it is
//! [`crate::session_argv`]; whether to wait for a first connection or join at once is decided
//! by temor-core's master machine from the answer here.

use std::collections::BTreeSet;
use std::path::Path;

use crate::argv::{ControlLayout, check_argv};
use crate::config::HostAlias;
use crate::error::SshError;
use crate::files::{CommandRunner, FileTools};
#[cfg(feature = "io")]
use crate::private_dir::ensure_private_dir;

/// Whether a host's master is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MasterProbe {
    /// `ssh -O check` found it: a session joins it without connecting again.
    Running,
    /// There is no master (or its socket is stale): the next session makes one.
    NotRunning,
}

/// Whether a master's socket appeared since `before` (the layout's
/// [`finished_sockets`](ControlLayout::finished_sockets) when the connection was started): the
/// master listens only once the host is verified and the person has logged in, so this is
/// "established" without asking `ssh -O check`.
pub fn socket_appeared(before: &BTreeSet<String>, now: &BTreeSet<String>) -> bool {
    now.difference(before).next().is_some()
}

impl MasterProbe {
    /// The probe for the exit code of `ssh -O check`: 0 means a master answered; anything else
    /// (255 when the socket is missing or dead) means there is none to join.
    pub fn from_exit(code: Option<i32>) -> Self {
        match code {
            Some(0) => Self::Running,
            _ => Self::NotRunning,
        }
    }
}

/// Makes the private runtime directory and the socket directory under it, both owned by this
/// user with mode 0700, as ssh needs before it can make a master. The runtime directory's
/// parent (`$XDG_RUNTIME_DIR`) must exist. Safe to call before every connection.
#[cfg(feature = "io")]
pub fn prepare_control_dirs(layout: &ControlLayout) -> Result<(), SshError> {
    ensure_private_dir(layout.runtime_dir())?;
    ensure_private_dir(&layout.socket_dir())
}

/// Asks whether the master for `alias` is running, with `ssh -O check` run by `runner`.
pub fn probe_master(
    runner: &dyn CommandRunner,
    tools: &FileTools,
    alias: &HostAlias,
    layout: &ControlLayout,
) -> Result<MasterProbe, SshError> {
    let program: &Path = &tools.ssh;
    let out = runner
        .run(program, &check_argv(alias, layout))
        .map_err(|source| SshError::Spawn {
            program: program.display().to_string(),
            source,
        })?;
    Ok(MasterProbe::from_exit(out.code))
}
