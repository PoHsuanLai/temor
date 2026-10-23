//! Starting the daemon from the ssh channel's process so that it outlives it.

use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::error::Error;

/// Runs `program args` with no terminal and no standard streams, in a process group of its own so
/// that the hangup sent to the ssh session's group when the channel closes does not reach it. The
/// child is not waited for. `env` is set over the process's own.
pub fn spawn_detached(
    program: &Path,
    args: &[&str],
    env: &[(&str, &std::ffi::OsStr)],
) -> Result<(), Error> {
    Command::new(program)
        .args(args)
        .envs(env.iter().copied())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map(drop)
        .map_err(|error| Error::io(format!("starting {}", program.display()), error))
}
