//! Everything that differs by OS, and the only place `cfg(target_os)` appears in this crate.
//!
//! - Linux and macOS: a non-blocking regular-file open.
//! - Other systems: a metadata-checked open. TODO(platform:windows) marks where the owner's
//!   machine takes over.
//!
//! The private runtime directory and the socket path limit are temor-shell's.

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use unix::open_regular;

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod unsupported;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub use unsupported::open_regular;
