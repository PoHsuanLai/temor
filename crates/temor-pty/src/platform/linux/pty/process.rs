//! The child's exit, and signals to its process group.

use std::io;
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;

use rustix::process::{Pid, Signal, kill_process_group};

use super::spawn::{ChildState, Pty};
use crate::error::PtyError;
use crate::pty::{ChildExit, SignalKind};

impl Pty {
    /// The program's exit, reaping it if it has ended; `None` while it runs.
    pub fn try_exit(&mut self) -> Result<Option<ChildExit>, PtyError> {
        if let ChildState::Reaped(exit) = self.state {
            return Ok(Some(exit));
        }
        let status = self.child.try_wait().map_err(PtyError::Io)?;
        Ok(status.map(|status| self.reaped(status)))
    }

    /// Blocks until the program ends, and reaps it.
    pub fn wait_exit(&mut self) -> Result<ChildExit, PtyError> {
        if let ChildState::Reaped(exit) = self.state {
            return Ok(exit);
        }
        let status = self.child.wait().map_err(PtyError::Io)?;
        Ok(self.reaped(status))
    }

    /// Sends `signal` to the program's whole process group. Fails with [`PtyError::Reaped`]
    /// once the program has been reaped, since its group id may then belong to someone else.
    pub fn signal_group(&self, signal: SignalKind) -> Result<(), PtyError> {
        if let ChildState::Reaped(_) = self.state {
            return Err(PtyError::Reaped);
        }
        let group = Pid::from_raw(self.pid().0)
            .ok_or_else(|| PtyError::Signal(io::Error::from(io::ErrorKind::InvalidInput)))?;
        match kill_process_group(group, rustix_signal(signal)) {
            Ok(()) => Ok(()),
            // The group is already empty: the leader is a zombie waiting to be reaped.
            Err(rustix::io::Errno::SRCH) => Ok(()),
            Err(errno) => Err(PtyError::Signal(errno.into())),
        }
    }

    /// Kills the program's process group and reaps the program.
    pub fn kill(&mut self) -> Result<ChildExit, PtyError> {
        match self.signal_group(SignalKind::Kill) {
            Ok(()) | Err(PtyError::Reaped) => self.wait_exit(),
            Err(error) => Err(error),
        }
    }

    fn reaped(&mut self, status: ExitStatus) -> ChildExit {
        let exit = match (status.code(), status.signal()) {
            (Some(code), _) => ChildExit::Exited(code),
            (None, Some(signal)) => ChildExit::Signaled(signal),
            (None, None) => ChildExit::Exited(-1),
        };
        self.state = ChildState::Reaped(exit);
        exit
    }
}

fn rustix_signal(signal: SignalKind) -> Signal {
    match signal {
        SignalKind::Hangup => Signal::HUP,
        SignalKind::Interrupt => Signal::INT,
        SignalKind::Terminate => Signal::TERM,
        SignalKind::Kill => Signal::KILL,
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        if let ChildState::Running = self.state {
            // Best effort: a shell passes the hangup on to its jobs; then make sure.
            let _ = self.signal_group(SignalKind::Hangup);
            let _ = self.kill();
        }
    }
}
