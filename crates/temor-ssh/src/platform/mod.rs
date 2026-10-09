//! Everything that differs by OS, and the only place `cfg(target_os)` appears in this crate.
//!
//! - Linux: a real private directory (mode 0700, owned by this user).
//! - Other systems: stubs returning [`crate::SshError::Unsupported`]. TODO(platform:macos) and
//!   TODO(platform:windows) mark where the owner's machines take over.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::ensure_private_dir;

#[cfg(not(target_os = "linux"))]
mod unsupported;
#[cfg(not(target_os = "linux"))]
pub use unsupported::ensure_private_dir;
