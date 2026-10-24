//! The real helper binary as a local daemon: it listens on a unix socket in a private folder under
//! the person's HOME (any unix, macOS included), a shell started through it keeps running when the
//! connection is gone, and the next connection finds it with its screen.

use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};

use temor_remote::Client;
use temor_remote::host::{DAEMON, HostEnv, LOCAL, dial_local, local_socket_path, runtime_dir};
use temor_testkit::Scratch;
use temor_wire::message::SessionState;

use crate::support::{Rig, eventually, snapshot_text, tools_present};

const HELPER_BIN: &str = env!("CARGO_BIN_EXE_temor-remote");

/// A daemon process over a scratch HOME. Killed when dropped.
struct Local {
    child: Child,
    scratch: Scratch,
}

impl Local {
    fn start() -> Self {
        // Under /tmp: a socket's path is limited (103 bytes on macOS), and the system temp folder
        // there is long.
        let scratch = Scratch::new_in(Path::new("/tmp"));
        let child = Command::new(HELPER_BIN)
            .args([DAEMON, LOCAL])
            .env_clear()
            .env("HOME", &scratch.home)
            .env("SHELL", "/bin/bash")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .expect("the daemon starts");
        // Connected only once it listens: a dial before that would start a daemon of its own.
        let dir = runtime_dir(None, Some(scratch.home.as_os_str())).expect("a socket folder");
        let socket = local_socket_path(&dir);
        eventually("the daemon's socket", || socket.exists().then_some(()));
        Self { child, scratch }
    }

    /// A client of the daemon, found through the same call the window makes.
    fn connect(&self) -> (Client, std::sync::mpsc::Receiver<temor_remote::ClientEvent>) {
        let env = HostEnv {
            xdg_runtime_dir: None,
            home: Some(self.scratch.home.clone().into()),
            linger: None,
            shell: Some("/bin/bash".into()),
            exe: HELPER_BIN.into(),
        };
        dial_local(&env).expect("the local daemon")
    }
}

impl Drop for Local {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn a_shell_started_through_the_local_daemon_outlives_its_connection() {
    if !tools_present() {
        return;
    }
    let rig = Rig::new("local");
    let local = Local::start();
    let (client, events) = local.connect();
    let id = client.open_session(rig.bash()).expect("open");
    client.attach(id).expect("attach");
    client
        .send_input(id, b"echo before-$((6*7))\n")
        .expect("input");
    let mut seen = Vec::new();
    eventually("the output", || {
        while let Ok(event) = events.try_recv() {
            if let temor_remote::ClientEvent::Output { bytes, .. } = event {
                seen.extend_from_slice(&bytes);
            }
        }
        String::from_utf8_lossy(&seen)
            .contains("before-42")
            .then_some(())
    });
    // The window goes away.
    drop(client);
    drop(events);

    let (again, _events) = local.connect();
    // The daemon notices the old connection is gone, and keeps the session.
    eventually("the session detached", || {
        let listed = again.list_sessions().ok()?;
        (listed.len() == 1 && listed[0].state == SessionState::Detached).then_some(())
    });
    let attached = eventually("the screen back", || {
        let attached = again.attach(id).ok()?;
        snapshot_text(&attached)
            .contains("before-42")
            .then_some(attached)
    });
    assert!(snapshot_text(&attached).contains("before-42"));
    again.close_session(id).expect("close");
}
