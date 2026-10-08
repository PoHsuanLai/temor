//! The pseudo-terminal: the portable values and launcher; the OS side lives in `platform`.

mod foreground;
mod launcher;
mod model;

pub use crate::platform::{Pty, PtyIo, run_session_leader};
pub use launcher::{SessionLauncher, find_program};
pub use model::{
    ChildExit, Foreground, Interest, ProcRoot, ProcessId, ReadOutcome, Readiness, Ready,
    SignalKind, SpawnSpec, WriteOutcome,
};
