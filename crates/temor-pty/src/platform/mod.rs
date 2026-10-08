//! Everything that differs by OS, and the only place `cfg(target_os)` appears.
//!
//! - Linux: the real pseudo-terminal (`linux/pty`) and process facts from `/proc`.
//! - macOS: stubs; spawning returns `PtyError::Unsupported` (TODO(platform:macos)).
//! - Windows and every other OS: the same stubs (TODO(platform:windows)).

mod os;

pub(crate) use os::Os;

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
