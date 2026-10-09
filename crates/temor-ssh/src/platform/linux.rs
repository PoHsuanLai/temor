//! Linux: the runtime directory is created 0700, or an existing one is checked and tightened,
//! all through an opened descriptor so nothing is followed by name after the check.

use std::fs::{DirBuilder, File, Permissions};
use std::io::{self, ErrorKind};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::Path;

use rustix::fs::{Mode, OFlags};

use crate::error::SshError;

const PRIVATE_MODE: u32 = 0o700;
const GROUP_OTHER_BITS: u32 = 0o077;

/// Makes `path` a directory only this user can enter. The parent must exist: for the runtime
/// directory that is `$XDG_RUNTIME_DIR`, and for `<runtime>/ssh` it is the runtime directory,
/// which the caller creates first. An existing directory must be a real directory owned by this
/// user; a group or other permission is removed with `fchmod` on the opened descriptor.
pub fn ensure_private_dir(path: &Path) -> Result<(), SshError> {
    match DirBuilder::new().mode(PRIVATE_MODE).create(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::AlreadyExists => check_existing(path),
        Err(source) => Err(io_error(path, source)),
    }
}

fn io_error(path: &Path, source: io::Error) -> SshError {
    SshError::Io {
        path: path.to_path_buf(),
        source,
    }
}

fn check_existing(path: &Path) -> Result<(), SshError> {
    // O_NOFOLLOW refuses a symlink in place of the directory; O_DIRECTORY refuses any file.
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let fd = rustix::fs::open(path, flags, Mode::empty()).map_err(|e| io_error(path, e.into()))?;
    let dir = File::from(fd);
    let meta = dir.metadata().map_err(|e| io_error(path, e))?;
    let me = rustix::process::getuid().as_raw();
    if meta.uid() != me {
        return Err(SshError::RuntimeDirNotPrivate {
            path: path.to_path_buf(),
            reason: "another user owns it",
        });
    }
    if meta.mode() & GROUP_OTHER_BITS != 0 {
        dir.set_permissions(Permissions::from_mode(PRIVATE_MODE))
            .map_err(|e| io_error(path, e))?;
    }
    Ok(())
}

/// Opens `path` for reading without blocking, and only if it is a regular file. A FIFO, a
/// device such as `/dev/zero`, or a directory is refused here, after a non-blocking open that
/// cannot hang, so a config `Include` can never stall or stream forever.
pub fn open_regular(path: &Path) -> io::Result<File> {
    let flags = OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC;
    let fd = rustix::fs::open(path, flags, Mode::empty()).map_err(io::Error::from)?;
    let file = File::from(fd);
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(ErrorKind::InvalidData, "not a regular file"));
    }
    Ok(file)
}
