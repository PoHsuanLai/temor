//! Linux: the runtime directory is created 0700, or an existing one is checked and tightened.

use std::fs::{self, DirBuilder, Permissions};
use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::Path;

use crate::error::SshError;

const PRIVATE_MODE: u32 = 0o700;
const GROUP_OTHER_BITS: u32 = 0o077;

/// Makes `path` a directory only this user can enter. The parent must exist (for the runtime
/// directory, `$XDG_RUNTIME_DIR` does). An existing directory must be a real directory, owned
/// by this user; a group or other permission is removed.
pub fn ensure_private_dir(path: &Path) -> Result<(), SshError> {
    match DirBuilder::new().mode(PRIVATE_MODE).create(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => check_existing(path),
        Err(source) => Err(SshError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn check_existing(path: &Path) -> Result<(), SshError> {
    let io_error = |source| SshError::Io {
        path: path.to_path_buf(),
        source,
    };
    // `symlink_metadata` so a symlink in place of the directory is refused, not followed.
    let meta = fs::symlink_metadata(path).map_err(io_error)?;
    if !meta.is_dir() {
        return Err(SshError::RuntimeDirNotPrivate {
            path: path.to_path_buf(),
            reason: "it is not a plain directory",
        });
    }
    let me = rustix::process::getuid().as_raw();
    if meta.uid() != me {
        return Err(SshError::RuntimeDirNotPrivate {
            path: path.to_path_buf(),
            reason: "another user owns it",
        });
    }
    if meta.mode() & GROUP_OTHER_BITS != 0 {
        fs::set_permissions(path, Permissions::from_mode(PRIVATE_MODE)).map_err(io_error)?;
    }
    Ok(())
}
