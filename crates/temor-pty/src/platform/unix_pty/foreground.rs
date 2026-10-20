//! The terminal's foreground process group, and its leader's name and directory.

use std::io;

use super::spawn::Pty;
use crate::error::PtyError;
use crate::pty::{Foreground, ProcRoot, ProcessId};

impl Pty {
    /// The foreground process group (`tcgetpgrp`), with its leader's name and working
    /// directory from `proc`; a pipeline is named for its last command.
    pub fn foreground(&self, proc: &ProcRoot) -> Result<Foreground, PtyError> {
        let group = foreground_group(&self.master)?;
        Ok(Foreground::of_pipeline(group, self.pid(), proc))
    }
}

/// The terminal's foreground process group. Linux reports a terminal with none as `ENOTTY`; macOS
/// answers 0 until the child has made itself the session leader, which rustix's `tcgetpgrp`
/// would turn into a panic, so this asks through nix and treats 0 as the same "none yet".
fn foreground_group(master: &std::os::fd::OwnedFd) -> Result<ProcessId, PtyError> {
    let group = nix::unistd::tcgetpgrp(master).map_err(|e| PtyError::Foreground(e.into()))?;
    match group.as_raw() {
        raw if raw > 0 => Ok(ProcessId(raw)),
        _ => Err(PtyError::Foreground(io::Error::from(
            io::ErrorKind::NotFound,
        ))),
    }
}
