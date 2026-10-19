//! Systems without private-directory support yet: a typed `Unsupported`, so the caller can
//! turn SSH off there and say why.
//!
//! TODO(platform:macos): create the runtime directory 0700 under the per-user temp dir (the
//! Linux module is the model; `fchmod` and `getuid` are the same calls).
//! TODO(platform:windows): an ACL that grants only this user, or no SSH control sockets at all.

use std::fs::File;
use std::io;
use std::path::Path;

use crate::error::SshError;

/// The longest unix socket path: `sun_path` is 104 bytes on macOS and the BSDs, including the
/// terminating NUL (Windows' is 108, so the smaller one holds everywhere here).
pub const SOCKET_PATH_MAX: usize = 103;

/// Not yet implemented on this system; always `Err(Unsupported)`.
pub fn ensure_private_dir(_path: &Path) -> Result<(), SshError> {
    Err(SshError::Unsupported("create a private runtime directory"))
}

/// Without a non-blocking open, only the metadata is checked before opening.
// TODO(platform:macos) and TODO(platform:windows): use the non-blocking open from linux.rs.
pub fn open_regular(path: &Path) -> io::Result<File> {
    if !std::fs::metadata(path)?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "not a regular file",
        ));
    }
    File::open(path)
}
