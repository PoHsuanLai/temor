//! Systems without private-folder support yet: a typed refusal, so the caller can say why.
//!
//! TODO(platform:macos): create the folder 0700 (the Linux module is the model; `fchmod` and
//! `getuid` are the same calls), and `SOCKET_PATH_MAX` stays 103 (`sun_path` is 104 bytes).
//! TODO(platform:windows): an ACL that grants only this user.

use std::io;
use std::path::Path;

use crate::error::ShellError;

/// The longest unix socket path: `sun_path` is 104 bytes on macOS and the BSDs, including the
/// terminating NUL (Windows' is 108, so the smaller one holds everywhere here).
pub const SOCKET_PATH_MAX: usize = 103;

/// Not yet implemented on this system; always an error.
pub fn ensure_private_dir(path: &Path) -> Result<(), ShellError> {
    Err(ShellError::PrivateDir {
        path: path.to_path_buf(),
        source: io::Error::new(
            io::ErrorKind::Unsupported,
            "private folders are not supported on this system yet",
        ),
    })
}
