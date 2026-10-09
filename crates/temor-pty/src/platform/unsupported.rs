//! Platforms without temor's pseudo-terminal support yet (macOS, Windows and the rest): the
//! same API, where spawning returns [`PtyError::Unsupported`] and so no value of these types
//! ever exists.
//!
//! TODO(platform:macos): a real pseudo-terminal (the Linux module is the model; `openpt` there
//! takes no CLOEXEC flag, and `/proc` facts come from `proc_pidinfo` instead).
//! TODO(platform:windows): ConPTY.

use std::convert::Infallible;
use std::ffi::OsString;
use std::time::Duration;

use crate::error::PtyError;
use crate::pty::SessionLauncher;
use crate::pty::{
    ChildExit, Foreground, Interest, ProcRoot, ProcessId, ReadOutcome, Readiness, SignalKind,
    SpawnSpec, WriteOutcome,
};
use crate::size::GridSize;

/// A pseudo-terminal; never constructed on this platform.
#[derive(Debug)]
pub struct Pty {
    never: Infallible,
}

/// The master side of a terminal; never constructed on this platform.
#[derive(Debug)]
pub struct PtyIo {
    never: Infallible,
}

impl Pty {
    // TODO(platform:macos), TODO(platform:windows): spawn on a real terminal.
    pub fn spawn(_spec: &SpawnSpec, _launcher: &SessionLauncher) -> Result<Self, PtyError> {
        Err(PtyError::Unsupported)
    }
    pub fn io(&self) -> Result<PtyIo, PtyError> {
        match self.never {}
    }
    pub fn resize(&self, _size: GridSize) -> Result<(), PtyError> {
        match self.never {}
    }
    pub fn pid(&self) -> ProcessId {
        match self.never {}
    }
    pub fn try_exit(&mut self) -> Result<Option<ChildExit>, PtyError> {
        match self.never {}
    }
    pub fn wait_exit(&mut self) -> Result<ChildExit, PtyError> {
        match self.never {}
    }
    pub fn signal_group(&self, _signal: SignalKind) -> Result<(), PtyError> {
        match self.never {}
    }
    pub fn kill(&mut self) -> Result<ChildExit, PtyError> {
        match self.never {}
    }
    pub fn foreground(&self, _proc: &ProcRoot) -> Result<Foreground, PtyError> {
        match self.never {}
    }
}

impl PtyIo {
    pub fn resize(&self, _size: GridSize) -> Result<(), PtyError> {
        match self.never {}
    }
    pub fn read(&self, _buf: &mut [u8]) -> Result<ReadOutcome, PtyError> {
        match self.never {}
    }
    pub fn write(&self, _bytes: &[u8]) -> Result<WriteOutcome, PtyError> {
        match self.never {}
    }
    pub fn wait(
        &self,
        _interest: Interest,
        _timeout: Option<Duration>,
    ) -> Result<Readiness, PtyError> {
        match self.never {}
    }
}

/// There is no session to lead on this platform.
// TODO(platform:macos): setsid, TIOCSCTTY and exec, as on Linux.
pub fn run_session_leader(_args: impl IntoIterator<Item = OsString>) -> PtyError {
    PtyError::Unsupported
}
