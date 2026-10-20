//! Everything that differs by OS, and the only place `cfg(target_os)` appears.
//!
//! - Linux and macOS: the real pseudo-terminal (`unix_pty`, shared).
//! - Linux: process facts from `/proc`. macOS: process facts from `proc_pidinfo` through darwin-libproc.
//! - Windows and every other OS: the same stubs (TODO(platform:windows)).

mod os;

pub(crate) use os::Os;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub(crate) use linux::Linux as Current;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub(crate) use macos::MacOs as Current;

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod windows;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub(crate) use windows::Windows as Current;

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix_pty;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use unix_pty::{Pty, PtyIo, run_session_leader};

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod unsupported;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub use unsupported::{Pty, PtyIo, run_session_leader};
