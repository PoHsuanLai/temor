//! Everything that differs by OS, and the only place `cfg(target_os)` appears here: the
//! controlling terminal as a transfer line, and the system's opener.
//!
//! - Linux: `/dev/tty` and `xdg-open`.
//! - macOS and Windows: stubs that say they are unsupported (TODO(platform:macos),
//!   TODO(platform:windows)).

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{SystemError, SystemOpener, open_tty};

#[cfg(not(target_os = "linux"))]
mod unsupported;
#[cfg(not(target_os = "linux"))]
pub use unsupported::{SystemError, SystemOpener, open_tty};
