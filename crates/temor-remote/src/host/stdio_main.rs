//! `temor-remote stdio`, what `ssh host temor-remote` runs: connect to the person's daemon
//! (starting it first when it is not running) and pass bytes between it and the ssh channel.

use std::io::{Read, Write};
use std::time::Duration;

use super::command::DAEMON;
use super::env::HostEnv;
use super::socket::{SocketSpot, socket_spot};
use crate::error::Error;
use crate::platform::{Half, connect, spawn_detached};

/// How long to wait for a daemon that was just started to listen.
const START_TRIES: u32 = 50;
const START_WAIT: Duration = Duration::from_millis(100);

/// Proxies stdin and stdout to the daemon until either side ends.
pub fn run_stdio(env: &HostEnv) -> Result<(), Error> {
    let SocketSpot { socket, .. } = socket_spot(env)?;

    let connection = match connect(&socket) {
        Ok(connection) => connection,
        Err(_) => {
            spawn_detached(&env.exe, &[DAEMON])?;
            wait_for(&socket)?
        }
    };
    proxy(connection)
}

pub(super) fn wait_for(socket: &std::path::Path) -> Result<Half, Error> {
    let mut last = Error::LinkClosed(None);
    for _ in 0..START_TRIES {
        std::thread::sleep(START_WAIT);
        match connect(socket) {
            Ok(connection) => return Ok(connection),
            Err(error) => last = error,
        }
    }
    Err(last)
}

fn proxy(connection: Half) -> Result<(), Error> {
    let mut up = connection.duplicate()?;
    std::thread::Builder::new()
        .name("temor-remote-stdin".to_string())
        .spawn(move || {
            // The channel closing (or the daemon going away) ends this direction; tell the daemon.
            let _ = std::io::copy(&mut std::io::stdin().lock(), &mut up);
            up.shutdown_write();
        })
        .map_err(|error| Error::io("starting the proxy", error))?;

    let mut down = connection;
    let mut stdout = std::io::stdout().lock();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = down
            .read(&mut buffer)
            .map_err(|error| Error::io("reading the daemon", error))?;
        if read == 0 {
            return Ok(());
        }
        stdout
            .write_all(&buffer[..read])
            .and_then(|()| stdout.flush())
            .map_err(|error| Error::io("writing to the channel", error))?;
    }
}
