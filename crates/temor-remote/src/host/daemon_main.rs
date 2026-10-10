//! The daemon process: listen on the socket, serve each connection on a thread, and tick.

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use temor_pty::SessionLauncher;
use temor_wire::units::UnixMillis;

use super::env::HostEnv;
use super::identity::hash_executable;
use super::paths::{runtime_dir, socket_path};
use crate::clock::SystemClock;
use crate::daemon::{Daemon, DaemonConfig};
use crate::error::Error;
use crate::idle::DEFAULT_IDLE;
use crate::platform::{Listener, connect, ensure_private_dir};

/// How often the daemon does its periodic work.
const TICK: Duration = Duration::from_secs(1);

/// The settings for a daemon run by `env`'s binary: its own `leader` command starts programs, so
/// the host needs nothing installed.
fn config(
    env: &HostEnv,
    helper: temor_wire::hello::HelperVersion,
    runtime_dir: PathBuf,
) -> DaemonConfig {
    let home = env.home.clone().filter(|home| !home.is_empty());
    DaemonConfig {
        helper,
        launcher: SessionLauncher::Trampoline {
            program: env.exe.clone(),
            args: vec![OsString::from("leader")],
        },
        default_shell: env
            .shell
            .clone()
            .filter(|shell| !shell.is_empty())
            .map_or_else(|| PathBuf::from("/bin/sh"), PathBuf::from),
        runtime_dir,
        file_roots: home.into_iter().map(PathBuf::from).collect(),
        proc_root: PathBuf::from("/proc"),
        idle: DEFAULT_IDLE,
        clock: Arc::new(SystemClock::new()),
    }
}

/// Clears uploads left half-staged by writers that are gone. On a thread of its own: the walk
/// may be long, and nothing waits for it. If the thread cannot start the files stay until the
/// next start.
fn sweep_in_background(daemon: &Arc<Daemon>) {
    let daemon = Arc::clone(daemon);
    let _ = std::thread::Builder::new()
        .name("temor-remote-sweep".to_string())
        .spawn(move || {
            let since = SystemTime::now().duration_since(UNIX_EPOCH);
            let Ok(since) = since else {
                return;
            };
            let now = i64::try_from(since.as_millis()).unwrap_or(i64::MAX);
            daemon.sweep_staged(UnixMillis(now));
        });
}

/// Runs the daemon until the process is killed. Returns at once, with `Ok`, if a daemon of this
/// version is already listening.
pub fn run_daemon(env: &HostEnv) -> Result<(), Error> {
    let helper = hash_executable(&env.exe)?;
    let dir = runtime_dir(env.xdg_runtime_dir.as_deref(), env.home.as_deref()).ok_or(
        Error::Unsupported {
            feature: "a private folder for the socket",
        },
    )?;
    ensure_private_dir(&dir)?;
    let socket = socket_path(&dir, &helper);
    if socket.exists() {
        if connect(&socket).is_ok() {
            return Ok(());
        }
        // Left by a daemon that died.
        std::fs::remove_file(&socket)
            .map_err(|error| Error::io("removing a stale socket", error))?;
    }
    let listener = Listener::bind(&socket)?;
    let daemon = Daemon::new(config(env, helper, dir));

    sweep_in_background(&daemon);

    let ticking = Arc::clone(&daemon);
    std::thread::Builder::new()
        .name("temor-remote-tick".to_string())
        .spawn(move || {
            loop {
                std::thread::sleep(TICK);
                ticking.tick();
            }
        })
        .map_err(|error| Error::io("starting the timer", error))?;

    loop {
        let connection = match listener.accept() {
            Ok(connection) => connection,
            Err(_) => {
                // Out of descriptors, say: wait rather than spin.
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
        };
        let writer = connection.duplicate()?;
        let serving = Arc::clone(&daemon);
        std::thread::Builder::new()
            .name("temor-remote-connection".to_string())
            .spawn(move || {
                // A connection that ends badly has nobody to tell; its sessions stay.
                let _ = serving.serve(connection, writer);
            })
            .map_err(|error| Error::io("starting a connection's thread", error))?;
    }
}
