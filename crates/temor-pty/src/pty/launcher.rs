//! How the child becomes a session leader with the terminal as its controlling terminal.

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

use crate::platform::{Current, Os};

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

/// The first executable file named `name` in the `PATH`-style list `path`.
pub fn find_program(name: &str, path: &OsStr) -> Option<PathBuf> {
    std::env::split_paths(path)
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.join(name))
        .find(|candidate| <Current as Os>::is_executable(candidate))
}
