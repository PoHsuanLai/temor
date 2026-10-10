//! The terminal's foreground process group, and its leader's name and directory.

use rustix::termios::tcgetpgrp;

use super::spawn::Pty;
use crate::error::PtyError;
use crate::pty::{Foreground, ProcRoot, ProcessId};

impl Pty {
    /// The foreground process group (`tcgetpgrp`), with its leader's name and working
    /// directory from `proc`; a pipeline is named for its last command.
    pub fn foreground(&self, proc: &ProcRoot) -> Result<Foreground, PtyError> {
        let group = ProcessId(
            tcgetpgrp(&self.master)
                .map_err(|e| PtyError::Foreground(e.into()))?
                .as_raw_pid(),
        );
        Ok(Foreground::of_pipeline(group, self.pid(), proc))
    }
}
