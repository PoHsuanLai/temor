//! The person's own machine as a host: connect to the local daemon, starting it first when it is
//! not running. The window uses this for sessions that outlive it; it is the same daemon and the
//! same client as for an ssh host, over the unix socket instead of a channel.

use std::io::{self, Write};
use std::sync::mpsc::Receiver;

use super::command::DAEMON;
use super::env::HostEnv;
use super::socket::{SocketSpot, socket_spot};
use super::stdio_main::wait_for;
use crate::client::{Client, ClientEvent};
use crate::error::Error;
use crate::platform::{Half, connect, spawn_detached};
use crate::protocol::{DAEMON_CAPS, hello};

/// A writer that tells the daemon no more is coming when it is dropped. The daemon ends a
/// connection on that and keeps its sessions; without it the reading half would hold the
/// connection open for as long as the daemon cared to wait.
struct HangUpOnDrop(Half);

impl Write for HangUpOnDrop {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl Drop for HangUpOnDrop {
    fn drop(&mut self) {
        self.0.shutdown_write();
    }
}

/// A client of the local daemon (started with `env`'s binary when none listens), and what the
/// daemon tells it unasked.
pub fn dial_local(env: &HostEnv) -> Result<(Client, Receiver<ClientEvent>), Error> {
    let SocketSpot { helper, socket, .. } = socket_spot(env)?;
    let half = match connect(&socket) {
        Ok(half) => half,
        Err(_) => {
            // The daemon lives by what the window was told, not by the process's own surroundings.
            let own = [
                ("HOME", env.home.as_deref()),
                ("XDG_RUNTIME_DIR", env.xdg_runtime_dir.as_deref()),
                ("SHELL", env.shell.as_deref()),
                ("TEMOR_REMOTE_LINGER", env.linger.as_deref()),
            ];
            let set: Vec<_> = own
                .into_iter()
                .filter_map(|(name, value)| Some((name, value?)))
                .collect();
            spawn_detached(&env.exe, &[DAEMON], &set)?;
            wait_for(&socket)?
        }
    };
    let writer = half.duplicate()?;
    Client::connect(half, HangUpOnDrop(writer), hello(helper, &DAEMON_CAPS))
}
