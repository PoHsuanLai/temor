//! How the child becomes a session leader with the terminal as its controlling terminal.

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

/// The exec step that runs `setsid` and `TIOCSCTTY` in the child before the program starts
/// (the workspace denies `unsafe`, so there is no `pre_exec` hook).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionLauncher {
    /// util-linux `setsid --ctty` at this path (Linux).
    Setsid(PathBuf),
    /// A program that calls [`run_session_leader`](crate::run_session_leader) with the
    /// arguments after `args`: this crate's `temor-pty-exec`, or temor's own binary with a
    /// hidden subcommand. Needs no util-linux.
    Trampoline {
        program: PathBuf,
        args: Vec<OsString>,
    },
}

/// The first file named `name` in the colon-separated `path` (a `PATH` value).
pub fn find_program(name: &str, path: &OsStr) -> Option<PathBuf> {
    path.to_string_lossy()
        .split(':')
        .filter(|dir| !dir.is_empty())
        .map(|dir| PathBuf::from(dir).join(name))
        .find(|candidate| candidate.is_file())
}
