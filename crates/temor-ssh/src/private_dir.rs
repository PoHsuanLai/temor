//! The private runtime directory, made by temor-shell (the one copy of that check) and reported
//! in this crate's words.

use std::path::Path;

use temor_shell::ShellError;

use crate::error::SshError;

/// Makes `path` a directory only this user can enter. The parent must exist: for the runtime
/// directory that is `$XDG_RUNTIME_DIR`, and for `<runtime>/ssh` it is the runtime directory,
/// which the caller creates first. An existing directory must be a real directory owned by this
/// user; any other mode is replaced by 0700.
pub fn ensure_private_dir(path: &Path) -> Result<(), SshError> {
    temor_shell::ensure_private_dir(path).map_err(|error| match error {
        ShellError::NotPrivate { path } => SshError::RuntimeDirNotPrivate {
            path,
            reason: "another user owns it",
        },
        ShellError::Unsupported { .. } => {
            SshError::Unsupported("create a private runtime directory")
        }
        ShellError::PrivateDir { path, source } => SshError::Io { path, source },
        other => SshError::Io {
            path: path.to_path_buf(),
            source: std::io::Error::other(other),
        },
    })
}
