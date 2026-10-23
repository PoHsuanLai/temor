//! Finding the daemon's socket: the helper's identity, the private folder and the path in it.
//! The daemon and the stdio proxy both start from here.

use std::path::PathBuf;

use temor_wire::hello::HelperVersion;

use super::env::HostEnv;
use super::identity::hash_executable;
use super::paths::{local_socket_path, runtime_dir, socket_path};
use crate::error::Error;
use crate::platform::ensure_private_dir;

/// Where the daemon of one helper build listens.
pub(super) struct SocketSpot {
    /// Who this helper is.
    pub(super) helper: HelperVersion,
    /// The private folder the socket is in, made if it was not there.
    pub(super) dir: PathBuf,
    /// The socket's path.
    pub(super) socket: PathBuf,
}

/// Which daemon a socket belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Whose {
    /// The daemon of this build, for a host reached over ssh.
    Helper,
    /// The person's own daemon on this computer, one for every build that speaks the protocol,
    /// so updating temor does not leave its sessions behind.
    Local,
}

/// The socket for the binary of `env`, with its private folder made.
pub(super) fn socket_spot(env: &HostEnv, whose: Whose) -> Result<SocketSpot, Error> {
    let helper = hash_executable(&env.exe)?;
    let dir = runtime_dir(env.xdg_runtime_dir.as_deref(), env.home.as_deref()).ok_or(
        Error::Unsupported {
            feature: "a private folder for the socket",
        },
    )?;
    ensure_private_dir(&dir)?;
    let socket = match whose {
        Whose::Helper => socket_path(&dir, &helper),
        Whose::Local => local_socket_path(&dir),
    };
    Ok(SocketSpot {
        helper,
        dir,
        socket,
    })
}
