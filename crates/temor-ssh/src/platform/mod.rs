//! Everything that differs by OS, and the only place `cfg(target_os)` appears in this crate.
//!
//! - Linux: a real private directory (mode 0700, owned by this user) and a non-blocking
//!   regular-file open.
//! - Other systems: stubs returning [`crate::SshError::Unsupported`], and a metadata-checked
//!   open. TODO(platform:macos) and TODO(platform:windows) mark where the owner's machines
//!   take over.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{ensure_private_dir, open_regular};

#[cfg(not(target_os = "linux"))]
mod unsupported;
#[cfg(not(target_os = "linux"))]
pub use unsupported::{ensure_private_dir, open_regular};
