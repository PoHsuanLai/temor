//! Everything that differs by OS, and the only place `cfg(target_os)` appears in this crate.
//!
//! - Linux: the private folder (0700, ours, checked through an opened descriptor) and the unix
//!   socket path limit.
//! - macOS: the same folder rules through `lstat` and `fchmodat`, and the 103-byte socket limit.
//! - Other systems: a typed refusal for the folder, and the smallest socket limit.
//!   TODO(platform:windows) marks where the owner's machine takes over.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{SOCKET_PATH_MAX, ensure_private_dir};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{SOCKET_PATH_MAX, ensure_private_dir};

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod unsupported;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub use unsupported::{SOCKET_PATH_MAX, ensure_private_dir};

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests;
