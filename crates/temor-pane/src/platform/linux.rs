//! Linux: `SO_PEERCRED`, and this process's user id. (The private socket folder is temor-shell's.)

use std::io;
use std::os::unix::net::UnixStream;

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
