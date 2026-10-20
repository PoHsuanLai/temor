//! macOS: a private folder is created 0700, or an existing one is checked and tightened (to 0700,
//! even from 0300 or 0000). There is no `O_PATH` or `/proc/self/fd` here, so a folder that cannot be
//! opened for reading is checked with `lstat` and tightened with `fchmodat` that does not follow a
//! link.

use std::fs::{DirBuilder, File, Permissions};
use std::io::{self, ErrorKind};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::Path;

use rustix::fs::{AtFlags, CWD, Mode, OFlags};
use rustix::io::Errno;

use crate::error::ShellError;

/// The longest unix socket path: `sun_path` is 104 bytes including the terminating NUL.
pub const SOCKET_PATH_MAX: usize = 103;

const PRIVATE_MODE: u32 = 0o700;
const MODE_BITS: u32 = 0o777;

/// Makes `path` a folder only this user can enter. The parent must exist. An existing folder must
/// be a real folder (not a link) owned by this user; any other mode is replaced by 0700.
pub fn ensure_private_dir(path: &Path) -> Result<(), ShellError> {
    match DirBuilder::new().mode(PRIVATE_MODE).create(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::AlreadyExists => check_existing(path),
        Err(source) => Err(io_error(path, source)),
    }
}

fn io_error(path: &Path, source: io::Error) -> ShellError {
    ShellError::PrivateDir {
        path: path.to_path_buf(),
        source,
    }
}

fn check_existing(path: &Path) -> Result<(), ShellError> {
    // O_NOFOLLOW refuses a symlink in place of the folder; O_DIRECTORY refuses any file.
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    match rustix::fs::open(path, flags, Mode::empty()) {
        Ok(fd) => {
            let dir = File::from(fd);
            let meta = dir.metadata().map_err(|e| io_error(path, e))?;
            check_owner(path, meta.uid())?;
            if meta.mode() & MODE_BITS != PRIVATE_MODE {
                dir.set_permissions(Permissions::from_mode(PRIVATE_MODE))
                    .map_err(|e| io_error(path, e))?;
            }
            Ok(())
        }
        // A folder of ours with no read permission (mode 0300 or 0000) cannot be opened, so it is
        // read by name without following a link, and tightened the same way.
        Err(Errno::ACCESS) => {
            let stat = rustix::fs::statat(CWD, path, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(|e| io_error(path, e.into()))?;
            if stat.st_mode as u32 & 0o170_000 != 0o040_000 {
                return Err(io_error(path, ErrorKind::NotADirectory.into()));
            }
            check_owner(path, stat.st_uid)?;
            rustix::fs::chmodat(
                CWD,
                path,
                Mode::from_raw_mode(PRIVATE_MODE as _),
                AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(|e| io_error(path, e.into()))
        }
        Err(errno) => Err(io_error(path, errno.into())),
    }
}

fn check_owner(path: &Path, uid: u32) -> Result<(), ShellError> {
    if uid != rustix::process::getuid().as_raw() {
        return Err(ShellError::NotPrivate {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}
