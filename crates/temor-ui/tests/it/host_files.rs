//! Files on a host through its helper (PLAN §9b): the copy the viewer and the peek card are made
//! from, against a helper in this process over pipes. Nothing here starts a shell.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Sender, channel};
use std::time::Duration;

use temor_core::host::edit::EditFault;
use temor_pty::SessionLauncher;
use temor_remote::{
    Client, Clock, DAEMON_CAPS, DEFAULT_IDLE, Daemon, DaemonConfig, ManualClock, hello,
};
use temor_ui::edge::remote_conn::Conn;
use temor_ui::edge::viewer_host::{Cap, CopyDone, CopyOutcome, copy_from_helper};
use temor_wire::digest::ContentHash;
use temor_wire::hello::HelperVersion;
use temor_wire::message::RemotePath;

use super::support::scratch;

const HELPER: HelperVersion = HelperVersion(ContentHash([5; 32]));
const PATIENCE: Duration = Duration::from_secs(20);

/// A helper in this process that reaches `root`, and a connection to it.
struct Host {
    conn: Arc<Conn>,
    root: PathBuf,
    // The client's events: kept so the link is not seen as closed.
    _events: std::sync::mpsc::Receiver<temor_remote::ClientEvent>,
}

fn host(name: &str) -> Host {
    let dir = scratch(name);
    let root = dir.join("home");
    let daemon = Daemon::new(DaemonConfig {
        helper: HELPER,
        launcher: SessionLauncher::Setsid("/usr/bin/setsid".into()),
        default_shell: "/bin/sh".into(),
        runtime_dir: dir.join("run"),
        home: Some(root.clone()),
        file_roots: vec![root.clone()],
        proc_root: "/proc".into(),
        idle: DEFAULT_IDLE,
        clock: Arc::new(ManualClock::new()) as Arc<dyn Clock>,
    });
    let (served_in, client_out) = std::io::pipe().expect("pipe");
    let (client_in, served_out) = std::io::pipe().expect("pipe");
    std::thread::spawn(move || daemon.serve(served_in, served_out));
    let (client, events) =
        Client::connect(client_in, client_out, hello(HELPER, &DAEMON_CAPS)).expect("greeting");
    Host {
        conn: Arc::new(Conn::bare(client)),
        root,
        _events: events,
    }
}

struct Tell(Sender<CopyOutcome>);

impl CopyDone for Tell {
    fn done(self: Box<Self>, outcome: CopyOutcome) {
        let _ = self.0.send(outcome);
    }
}

/// Copies `path` of the host to `local` and waits for the outcome.
fn copy(host: &Host, path: &Path, local: &Path, cap: Cap) -> CopyOutcome {
    let (tell, heard) = channel();
    let remote = RemotePath(path.display().to_string());
    copy_from_helper(
        Box::new(Tell(tell)),
        Arc::clone(&host.conn),
        (remote, local.to_path_buf()),
        cap,
    );
    heard.recv_timeout(PATIENCE).expect("the copy reports")
}

#[test]
fn a_file_of_the_host_is_copied_whole_or_as_its_first_bytes_and_true_length() {
    let host = host("host-files-copy");
    let text: Vec<u8> = (0..5000u32).map(|i| b'a' + (i % 26) as u8).collect();
    std::fs::write(host.root.join("big.txt"), &text).expect("file");
    let local = host.root.parent().expect("scratch").join("copy.txt");

    let whole = copy(&host, &host.root.join("big.txt"), &local, Cap::Unlimited);
    assert_eq!(whole, CopyOutcome::Whole);
    assert_eq!(std::fs::read(&local).expect("copy"), text);

    // Under the cap: whole. Over it: the first bytes in a file as long as the original.
    let fits = copy(&host, &host.root.join("big.txt"), &local, Cap::Bytes(5000));
    assert_eq!(fits, CopyOutcome::Whole);
    let over = copy(&host, &host.root.join("big.txt"), &local, Cap::Bytes(4999));
    assert_eq!(over, CopyOutcome::Header { len: 5000 });
    let header = std::fs::read(&local).expect("copy");
    assert_eq!(header.len(), 5000);
    assert_eq!(header[..100], text[..100]);
}

#[test]
fn what_is_not_a_file_there_leaves_no_copy_and_says_why() {
    let host = host("host-files-not");
    std::fs::create_dir(host.root.join("dir")).expect("folder");
    let local = host.root.parent().expect("scratch").join("copy.txt");
    let outside = host.root.parent().expect("scratch").join("secret.txt");
    std::fs::write(&outside, "x").expect("file outside what the helper reaches");
    let cases = [
        ("a folder", host.root.join("dir"), CopyOutcome::NotAFile),
        ("missing", host.root.join("none.txt"), CopyOutcome::Gone),
        (
            "outside the folders it was given",
            outside,
            CopyOutcome::Failed(EditFault::Refused),
        ),
    ];
    for (why, path, want) in cases {
        assert_eq!(copy(&host, &path, &local, Cap::Unlimited), want, "{why}");
        assert!(!local.exists(), "{why}: no copy is left");
    }
}
