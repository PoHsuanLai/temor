//! Systems without private-directory support yet: a typed `Unsupported`, so the caller can
//! turn SSH off there and say why.
//!
//! TODO(platform:macos): create the runtime directory 0700 under the per-user temp dir (the
//! Linux module is the model; `chmod` and `getuid` are the same calls).
//! TODO(platform:windows): an ACL that grants only this user, or no SSH control sockets at all.

use std::path::Path;

use crate::error::SshError;

/// Not yet implemented on this system; always `Err(Unsupported)`.
pub fn ensure_private_dir(_path: &Path) -> Result<(), SshError> {
    Err(SshError::Unsupported("create a private runtime directory"))
}
