//! Other systems: nothing is created and no peer is trusted, so no connection is accepted.
//!
//! TODO(platform:macos): create the directory 0700 under the per-user temp dir; read the peer
//! with `getpeereid` (or `LOCAL_PEERCRED`).
//! TODO(platform:windows): named pipes with an ACL for the current user replace the socket.

use std::io;
use std::os::unix::net::UnixStream;
use std::path::Path;

fn unsupported() -> io::Error {
    io::Error::new(io::ErrorKind::Unsupported, "not supported on this system yet")
}

pub(crate) fn ensure_private_dir(_path: &Path) -> io::Result<()> {
    Err(unsupported())
}

pub(crate) fn own_uid() -> u32 {
    u32::MAX
}

pub(crate) fn peer_uid(_stream: &UnixStream) -> io::Result<u32> {
    Err(unsupported())
}
