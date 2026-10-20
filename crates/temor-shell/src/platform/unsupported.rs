//! Systems without private-folder support yet: a typed refusal, so the caller can say why.
//!
//! TODO(platform:windows): an ACL that grants only this user.

use std::path::Path;

use crate::error::ShellError;

/// The longest unix socket path: `sun_path` is 104 bytes on macOS and the BSDs, including the
/// terminating NUL (Windows' is 108, so the smaller one holds everywhere here).
pub const SOCKET_PATH_MAX: usize = 103;

/// Not yet implemented on this system; always an error.
pub fn ensure_private_dir(path: &Path) -> Result<(), ShellError> {
    Err(ShellError::Unsupported {
        path: path.to_path_buf(),
    })
}
