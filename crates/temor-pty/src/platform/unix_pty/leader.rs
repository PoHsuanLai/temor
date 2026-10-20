//! The session-leader trampoline: the exec step a `SessionLauncher::Trampoline` runs.

use std::ffi::OsString;
use std::os::fd::AsFd;
use std::os::unix::process::CommandExt;
use std::process::Command;

use rustix::process::{ioctl_tiocsctty, setsid};

use crate::error::PtyError;

/// Makes this process a new session whose controlling terminal is its stdin, then replaces it
/// with `args[0]` run with the rest of `args`, keeping the environment and directory. Returns
/// only on failure. temor's binary (or `temor-pty-exec`) calls this with its arguments after
/// the ones it was given in [`SessionLauncher::Trampoline`](crate::SessionLauncher).
pub fn run_session_leader(args: impl IntoIterator<Item = OsString>) -> PtyError {
    let mut args = args.into_iter();
    let Some(program) = args.next() else {
        return PtyError::NoProgram;
    };
    if let Err(errno) = setsid() {
        return PtyError::SessionLeader(errno.into());
    }
    if let Err(errno) = ioctl_tiocsctty(std::io::stdin().as_fd()) {
        return PtyError::SessionLeader(errno.into());
    }
    let source = Command::new(&program).args(args).exec();
    PtyError::Spawn {
        program: program.into(),
        source,
    }
}
