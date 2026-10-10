//! What a daemon is made with. Nothing ambient is read: the clock, the shell, the launcher and
//! the `/proc` root are all handed in.

use std::path::PathBuf;
use std::sync::Arc;

use temor_pty::SessionLauncher;
use temor_wire::hello::HelperVersion;
use temor_wire::message::IdleLimit;

use crate::clock::Clock;

/// The daemon's settings.
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    /// This helper's identity (the hash of its binary), sent in its greeting.
    pub helper: HelperVersion,
    /// How a session's program becomes a session leader on its terminal: util-linux `setsid`, or
    /// this binary's own `leader` command.
    pub launcher: SessionLauncher,
    /// The program for a session that names none: the person's login shell.
    pub default_shell: PathBuf,
    /// The daemon's private folder (mode 0700). Shell integration shims go in a subfolder.
    pub runtime_dir: PathBuf,
    /// The folders the file service may reach before any session is opened.
    pub file_roots: Vec<PathBuf>,
    /// Where the process filesystem is mounted (normally `/proc`).
    pub proc_root: PathBuf,
    /// How long a detached session is kept; the client may change it.
    pub idle: IdleLimit,
    /// The time source.
    pub clock: Arc<dyn Clock>,
}
