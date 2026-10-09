//! `temor-remote stdio`, what `ssh host temor-remote` runs: connect to the person's daemon
//! (starting it first when it is not running) and pass bytes between it and the ssh channel.

use std::io::{Read, Write};
use std::time::Duration;

use super::env::HostEnv;
use super::identity::hash_executable;
use super::paths::{runtime_dir, socket_path};
use crate::error::Error;
use crate::platform::{Half, connect, ensure_private_dir, spawn_detached};

/// How long to wait for a daemon that was just started to listen.
const START_TRIES: u32 = 50;
const START_WAIT: Duration = Duration::from_millis(100);

/// Proxies stdin and stdout to the daemon until either side ends.
pub fn run_stdio(env: &HostEnv) -> Result<(), Error> {
    let helper = hash_executable(&env.exe)?;
    let dir = runtime_dir(env.xdg_runtime_dir.as_deref(), env.home.as_deref()).ok_or(
        Error::Unsupported {
            feature: "a private folder for the socket",
        },
    )?;
    ensure_private_dir(&dir)?;
    let socket = socket_path(&dir, &helper);

    let connection = match connect(&socket) {
        Ok(connection) => connection,
        Err(_) => {
            spawn_detached(&env.exe, &["daemon"])?;
            wait_for(&socket)?
        }
    };
    proxy(connection)
}

fn wait_for(socket: &std::path::Path) -> Result<Half, Error> {
    let mut last = Error::LinkClosed;
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
