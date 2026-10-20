//! Everything that differs by OS, and the only place `cfg(target_os)` appears in this crate.
//!
//! - Linux: a non-blocking regular-file open.
//! - Other systems: a metadata-checked open. TODO(platform:macos) and TODO(platform:windows) mark
//!   where the owner's machines take over.
//!
//! The private runtime directory and the socket path limit are temor-shell's.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::open_regular;

#[cfg(not(target_os = "linux"))]
mod unsupported;
#[cfg(not(target_os = "linux"))]
pub use unsupported::open_regular;
