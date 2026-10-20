//! Everything that differs by OS, and the only place `cfg(target_os)` appears in this crate.
//!
//! - Linux: the private folder (0700, ours, checked through an opened descriptor) and the unix
//!   socket path limit.
//! - Other systems: a typed refusal for the folder, and the smallest socket limit.
//!   TODO(platform:macos) and TODO(platform:windows) mark where the owner's machines take over.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{SOCKET_PATH_MAX, ensure_private_dir};

#[cfg(not(target_os = "linux"))]
mod unsupported;
#[cfg(not(target_os = "linux"))]
pub use unsupported::{SOCKET_PATH_MAX, ensure_private_dir};

#[cfg(all(test, target_os = "linux"))]
mod tests;
