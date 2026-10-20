//! Everything that differs by OS, and the only place `cfg(target_os)` appears here: the
//! controlling terminal as a transfer line, and the system's opener.
//!
//! - Linux and macOS: `/dev/tty` and `xdg-open` or `open`.
//! - Windows and the rest: stubs that say they are unsupported (TODO(platform:windows)).

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use unix::{SystemError, SystemOpener, open_tty};

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod unsupported;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub use unsupported::{SystemError, SystemOpener, open_tty};
