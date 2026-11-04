//! What the window's SSH needs from its surroundings, read once at start: where `ssh`, `scp` and
//! `ssh-keygen` are, where the shared connection's sockets and the local copies of edited files
//! live, and where `~/.ssh/config` is. Nothing here touches the network; the programs run only
//! when the hub asks (see `ssh_work`).

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use temor_ssh::{
    CommandOutput, CommandRunner, ConfigPaths, ControlLayout, ControlPersist, FileTools, HostList,
    RuntimeDir, Seconds, StdRunner,
};

use crate::ssh::hosts_cache::HostCache;

/// How long a shared connection outlives its last pane.
const PERSIST_SECONDS: u32 = 600;

/// A command runner that can be shared with the worker threads.
#[derive(Clone)]
pub struct ArcRunner(pub Arc<dyn CommandRunner + Send + Sync>);

impl std::fmt::Debug for ArcRunner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ArcRunner")
    }
}

impl Default for ArcRunner {
    fn default() -> Self {
        Self(Arc::new(StdRunner))
    }
}

impl CommandRunner for ArcRunner {
    fn run(&self, program: &std::path::Path, args: &[String]) -> std::io::Result<CommandOutput> {
        self.0.run(program, args)
    }
}

/// The SSH surroundings. Cloning shares the runner.
#[derive(Debug, Clone)]
pub struct SshEnv {
    /// The shared connection's sockets; the reason as text when they cannot be placed (a runtime
    /// folder whose path is too long for a socket).
    pub layout: Result<ControlLayout, String>,
    pub tools: FileTools,
    /// `ssh-keygen`, for the host key fingerprint.
    pub keygen: PathBuf,
    pub runner: ArcRunner,
    pub config: ConfigPaths,
    /// Where local copies of edited files are kept (private to the person).
    pub edit_dir: PathBuf,
    /// The host list as last read; shared by clones.
    hosts: HostCache,
}

impl SshEnv {
    /// The surroundings of a process whose `PATH` is `path`, home `home` and runtime folder
    /// `runtime_dir`: the programs found on that `PATH`, sockets under the runtime folder, the
    /// config at `~/.ssh/config`.
    pub fn new(path: Option<&OsStr>, home: &Path, runtime_dir: &Path) -> Self {
        let path = path.unwrap_or(OsStr::new("/usr/bin:/bin")).to_owned();
        let find = |name: &str| temor_pty::find_program(name, &path).unwrap_or_else(|| name.into());
        let layout = RuntimeDir::new(runtime_dir)
            .and_then(|runtime| {
                let persist = Seconds::new(PERSIST_SECONDS).map(ControlPersist::Idle);
                ControlLayout::new(runtime, persist.unwrap_or(ControlPersist::Forever))
            })
            .map_err(|err| err.to_string());
        Self {
            layout,
            tools: FileTools {
                ssh: find("ssh"),
                scp: find("scp"),
            },
            keygen: find("ssh-keygen"),
            runner: ArcRunner::default(),
            config: ConfigPaths::beside(home.join(".ssh").join("config"), Some(home.to_path_buf())),
            edit_dir: runtime_dir.join("edit"),
            hosts: HostCache::default(),
        }
    }

    /// The hosts of the person's `~/.ssh/config` (never fails; trouble in the file is skipped).
    /// The file is read again only when its modified time (or its folder's) moved.
    pub fn host_list(&self) -> HostList {
        self.hosts.list(&self.config)
    }
}
