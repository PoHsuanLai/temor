//! Linux: a private folder is created 0700, or an existing one is checked and tightened (to 0700,
//! even from 0300 or 0000), all through an opened descriptor so nothing is followed by name after
//! the check.

use std::fs::{DirBuilder, File, Permissions};
use std::io::{self, ErrorKind};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::Path;

use rustix::fs::{Mode, OFlags};
use rustix::io::Errno;

use crate::error::ShellError;

const PRIVATE_MODE: u32 = 0o700;
const MODE_BITS: u32 = 0o777;

/// Makes `path` a folder only this user can enter. The parent must exist. An existing folder must
/// be a real folder (not a link) owned by this user; any other mode is replaced by 0700 through
/// the opened descriptor.
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
        Ok(fd) => tighten(path, &File::from(fd)),
        // A folder of ours with no read permission (mode 0300 or 0000): open it without
        // reading, so it can still be checked and tightened.
        Err(Errno::ACCESS) => {
            let flags = OFlags::PATH | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
            let fd = rustix::fs::open(path, flags, Mode::empty())
                .map_err(|e| io_error(path, e.into()))?;
            tighten(path, &File::from(fd))
        }
        Err(errno) => Err(io_error(path, errno.into())),
    }
}

/// Checks that `dir` (opened without following a link) is ours, and makes it 0700 if it is
/// anything else. The change goes through the descriptor's `/proc` entry, which also works for a
/// descriptor opened without read permission.
fn tighten(path: &Path, dir: &File) -> Result<(), ShellError> {
    let meta = dir.metadata().map_err(|e| io_error(path, e))?;
    if meta.uid() != rustix::process::getuid().as_raw() {
        return Err(ShellError::NotPrivate {
            path: path.to_path_buf(),
        });
    }
    if meta.mode() & MODE_BITS != PRIVATE_MODE {
        let private = Permissions::from_mode(PRIVATE_MODE);
        // `fchmod` refuses a descriptor opened without read permission; its `/proc` entry works.
        if dir.set_permissions(private.clone()).is_err() {
            let held = format!("/proc/self/fd/{}", dir.as_raw_fd());
            std::fs::set_permissions(held, private).map_err(|e| io_error(path, e))?;
        }
    }
    Ok(())
}
