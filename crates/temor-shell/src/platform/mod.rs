//! Everything that differs by OS, and the only place `cfg(target_os)` appears in this crate.
//!
//! - Linux: the private folder (0700, ours, checked through an opened descriptor) and the unix
//!   socket path limit.
//! - macOS: the same folder rules through `lstat` and `fchmodat`, and the 103-byte socket limit.
//! - Other systems: a typed refusal for the folder, and the smallest socket limit.
//!   TODO(platform:windows) marks where the owner's machine takes over.

/// The longest unix socket path: `sun_path` is 108 bytes including the terminating NUL.
#[cfg(target_os = "linux")]
pub const SOCKET_PATH_MAX: usize = 107;
/// The longest unix socket path: `sun_path` is 104 bytes including the terminating NUL.
#[cfg(target_os = "macos")]
pub const SOCKET_PATH_MAX: usize = 103;
/// The longest unix socket path: `sun_path` is 104 bytes on macOS and the BSDs, including the
/// terminating NUL (Windows' is 108, so the smaller one holds everywhere here).
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub const SOCKET_PATH_MAX: usize = 103;

#[cfg(all(feature = "io", target_os = "linux"))]
mod linux;
#[cfg(all(feature = "io", target_os = "linux"))]
pub use linux::ensure_private_dir;

#[cfg(all(feature = "io", target_os = "macos"))]
mod macos;
#[cfg(all(feature = "io", target_os = "macos"))]
pub use macos::ensure_private_dir;

#[cfg(all(feature = "io", not(any(target_os = "linux", target_os = "macos"))))]
mod unsupported;
#[cfg(all(feature = "io", not(any(target_os = "linux", target_os = "macos"))))]
pub use unsupported::ensure_private_dir;

#[cfg(all(test, feature = "io", any(target_os = "linux", target_os = "macos")))]
mod tests;
