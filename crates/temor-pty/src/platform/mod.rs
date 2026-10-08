//! Everything that differs by OS, and the only place `cfg(target_os)` appears.
//!
//! - Linux: the real pseudo-terminal (`linux/pty`) and process facts from `/proc`.
//! - macOS: stubs; spawning returns `PtyError::Unsupported` (TODO(platform:macos)).
//! - Windows and every other OS: the same stubs (TODO(platform:windows)).

use std::path::PathBuf;

use crate::pty::{ProcRoot, ProcessId};
use crate::shell::UserId;

/// The process facts temor reads from the OS for tab titles, the running row and shell
/// resolution. Each OS module implements it; [`Current`] is this build's.
pub(crate) trait Os {
    /// The short name (`comm`) of process `pid`, when the OS can tell.
    fn process_name(proc: &ProcRoot, pid: ProcessId) -> Option<String>;
    /// The working directory of process `pid`, when the OS can tell.
    fn process_cwd(proc: &ProcRoot, pid: ProcessId) -> Option<PathBuf>;
    /// The real user id of this process.
    fn current_user() -> Option<UserId>;
}

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub(crate) use linux::Linux as Current;
#[cfg(target_os = "linux")]
pub use linux::{Pty, PtyIo, run_session_leader};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub(crate) use macos::MacOs as Current;

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod windows;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub(crate) use windows::Windows as Current;

#[cfg(not(target_os = "linux"))]
mod unsupported;
#[cfg(not(target_os = "linux"))]
pub use unsupported::{Pty, PtyIo, run_session_leader};
