//! Linux: a private directory made or checked through an opened descriptor, and `SO_PEERCRED`.

use std::fs::{DirBuilder, File, Permissions};
use std::io::{self, ErrorKind};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::Path;

use rustix::fs::{Mode, OFlags};

const PRIVATE_MODE: u32 = 0o700;
const GROUP_OTHER_BITS: u32 = 0o077;

/// Makes `path` a directory only this user can enter. The parent must exist. An existing
/// directory must be a real directory (not a link) owned by this user; a group or other
/// permission is removed with `fchmod` on the opened descriptor.
pub(crate) fn ensure_private_dir(path: &Path) -> io::Result<()> {
    match DirBuilder::new().mode(PRIVATE_MODE).create(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::AlreadyExists => check_existing(path),
        Err(err) => Err(err),
    }
}

fn check_existing(path: &Path) -> io::Result<()> {
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let fd = rustix::fs::open(path, flags, Mode::empty()).map_err(io::Error::from)?;
    let dir = File::from(fd);
    let meta = dir.metadata()?;
    if meta.uid() != own_uid() {
        return Err(io::Error::new(
            ErrorKind::PermissionDenied,
            "another user owns the directory",
        ));
    }
    if meta.mode() & GROUP_OTHER_BITS != 0 {
        dir.set_permissions(Permissions::from_mode(PRIVATE_MODE))?;
    }
    Ok(())
}

/// This process's user id.
pub(crate) fn own_uid() -> u32 {
    rustix::process::getuid().as_raw()
}

/// The user id of the process on the other end of `stream`, as the kernel recorded it when the
/// connection was made.
pub(crate) fn peer_uid(stream: &UnixStream) -> io::Result<u32> {
    let cred = rustix::net::sockopt::socket_peercred(stream).map_err(io::Error::from)?;
    Ok(cred.uid.as_raw())
}
