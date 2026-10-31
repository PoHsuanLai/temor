//! The pseudo-terminal: the portable values and launcher; the OS side lives in `platform`.

mod foreground;
mod launcher;
mod model;
mod shell;
mod user;

pub use crate::platform::{Pty, PtyIo, run_session_leader};
// A process's command line is data, kept in temor-schema with the rest (PLAN §2b).
pub use launcher::{SessionLauncher, find_program};
pub use model::{
    ChildExit, Foreground, Interest, ProcRoot, ProcessId, ReadOutcome, Readiness, Ready,
    SignalKind, SpawnSpec, WriteOutcome,
};
pub use shell::prepare_shell;
pub use temor_schema::process::CommandLine;
#[cfg(target_os = "linux")]
pub(crate) use temor_schema::process::MAX_BYTES;
pub use user::{current_user, login_shell};
