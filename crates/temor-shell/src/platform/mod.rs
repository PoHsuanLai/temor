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

/// [`ensure_private_dir`] for a folder whose parents may be missing: they are made first, 0700 on
/// unix. The error is the folder's: [`std::io::ErrorKind::PermissionDenied`] when one that is
/// there is not this user's own, [`std::io::ErrorKind::Unsupported`] where the system has no
/// check yet.
#[cfg(feature = "io")]
pub fn ensure_private_dir_all(path: &std::path::Path) -> std::io::Result<()> {
    use std::io::{Error, ErrorKind};

    use crate::error::ShellError;

    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        let mut parents = std::fs::DirBuilder::new();
        parents.recursive(true);
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut parents, 0o700);
        parents.create(parent)?;
    }
    ensure_private_dir(path).map_err(|error| match error {
        ShellError::PrivateDir { source, .. } => source,
        ShellError::NotPrivate { .. } => Error::new(
            ErrorKind::PermissionDenied,
            "the folder is not this user's own",
        ),
        ShellError::Unsupported { .. } => Error::new(ErrorKind::Unsupported, error.to_string()),
        other => Error::other(other),
    })
}
