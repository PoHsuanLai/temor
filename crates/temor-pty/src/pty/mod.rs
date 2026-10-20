//! The pseudo-terminal: the portable values and launcher; the OS side lives in `platform`.

mod command;
#[cfg(test)]
mod command_tests;
mod foreground;
mod launcher;
mod model;
mod shell;
mod user;

pub use crate::platform::{Pty, PtyIo, run_session_leader};
pub use command::CommandLine;
#[cfg(target_os = "linux")]
pub(crate) use command::MAX_BYTES;
pub use launcher::{SessionLauncher, find_program};
pub use model::{
    ChildExit, Foreground, Interest, ProcRoot, ProcessId, ReadOutcome, Readiness, Ready,
    SignalKind, SpawnSpec, WriteOutcome,
};
pub use shell::prepare_shell;
pub use user::{current_user, login_shell};
