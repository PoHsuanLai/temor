//! A pane on a host's helper: what the helper reads of its folder (the folder, the branch, the
//! changes) reaches the chips as a pane on this computer's facts do. The helper is a real one in
//! this process over pipes; nothing here reaches a network.

use std::path::Path;
use std::process::Command;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use temor_core::facts::FactsIn;
use temor_pty::SessionLauncher;
use temor_remote::{
    Client, ClientEvent, Clock, DAEMON_CAPS, DEFAULT_IDLE, Daemon, DaemonConfig, ManualClock, hello,
};
use temor_ssh::HostAlias;
use temor_wire::digest::ContentHash;
use temor_wire::hello::HelperVersion;
use temor_wire::message::{EnvVar, OpenSession, RemotePath};
use temor_wire::units::{CellPx, Cols, Rows, Size};

use super::support::Rig;
use crate::edge::remote_conn::Conn;
use crate::edge::remote_link::{RemoteLink, Target};
use crate::hub::remote::{RemotePane, Shown};
use crate::view::fact_chips::{ChipKind, chip_texts, lead_chips};
use crate::view::remote_launch::open_fed_session;

const HELPER: HelperVersion = HelperVersion(ContentHash([4; 32]));

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A helper in this process serving `home`, a connection to it, and a session in `cwd`.
fn helper_session(
    rig: &Rig,
    home: &Path,
    cwd: &Path,
) -> (Arc<Conn>, temor_wire::ids::SessionId, Receiver<ClientEvent>) {
    let setsid =
        temor_pty::find_program("setsid", std::ffi::OsStr::new("/usr/bin:/bin")).expect("setsid");
    let daemon = Daemon::new(DaemonConfig {
        helper: HELPER,
        launcher: SessionLauncher::Setsid(setsid),
        default_shell: "/bin/sh".into(),
        runtime_dir: rig.dir.join("run"),
        home: Some(home.to_path_buf()),
        file_roots: vec![home.to_path_buf()],
        proc_root: "/proc".into(),
        idle: DEFAULT_IDLE,
        clock: Arc::new(ManualClock::new()) as Arc<dyn Clock>,
    });
    let (served_in, client_out) = std::io::pipe().expect("pipe");
    let (client_in, served_out) = std::io::pipe().expect("pipe");
    std::thread::spawn(move || daemon.serve(served_in, served_out));
    let (client, events) =
        Client::connect(client_in, client_out, hello(HELPER, &DAEMON_CAPS)).expect("greeting");
    let env = [
        ("HOME", home.display().to_string()),
        ("PATH", "/usr/bin:/bin".to_owned()),
    ];
    let session = client
        .open_session(OpenSession {
            shell: Some(RemotePath("/bin/sh".to_owned())),
            cwd: RemotePath(cwd.display().to_string()),
            env: env
                .into_iter()
                .map(|(name, value)| EnvVar {
                    name: name.to_owned(),
                    value,
                })
                .collect(),
            size: Size {
                cols: Cols(80),
                rows: Rows(24),
            },
            cell: CellPx::DEFAULT,
            palette: None,
        })
        .expect("a session");
    (Arc::new(Conn::bare(client)), session, events)
}

#[test]
fn a_host_panes_folder_branch_and_changes_from_its_helper_show_as_its_chips() {
    let Some(mut rig) = Rig::new("host-chips") else {
        return;
    };
    if Command::new("git").arg("--version").output().is_err() {
        eprintln!("SKIPPED: needs git");
        return;
    }
    let home = rig.dir.join("hosthome");
    let repo = home.join("app");
    std::fs::create_dir_all(&repo).expect("the host's folders");
    git(&repo, &["init", "-q", "-b", "main"]);
    std::fs::write(repo.join("a.txt"), "one\n").expect("file");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-q", "-m", "first"]);
    std::fs::write(repo.join("a.txt"), "one\ntwo\n").expect("a change");
    let (conn, session, _events) = helper_session(&rig, &home, &repo);

    // A host pane that is attached to a session on the helper (the begin the connection makes).
    rig.hub.open_host_named("fake");
    let pane = rig.front();
    let alias = HostAlias::new("fake").expect("a name");
    let target = Arc::new(Mutex::new(Target::default()));
    let link = RemoteLink::start(Arc::clone(&target)).expect("the writer thread starts");
    let (_, feeder) = open_fed_session(
        &rig.hub.launch,
        rig.hub.palette,
        pane,
        rig.hub.events.clone(),
        link,
    )
    .expect("a fed session");
    rig.hub.remote.panes.insert(
        pane,
        RemotePane {
            alias: alias.clone(),
            target,
            feeder,
            session: Some(session),
            shown: Shown::Attached,
        },
    );
    rig.hub.remote.lines.entry(alias).or_default().conn = Some(conn);

    // The first prompt asks for a read; the window's loop would carry the result in.
    let mut inbox = rig.hub.facts.take_inbox().expect("the facts inbox");
    rig.hub.facts_in(pane, FactsIn::Prompt);
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        match inbox.try_recv() {
            Ok(ready) => {
                let _ = rig.hub.facts_ready(ready);
                break;
            }
            Err(_) => {
                assert!(Instant::now() < until, "the helper's facts never came");
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }

    let folder = rig
        .hub
        .chip_folder(pane)
        .expect("the folder as its chip says it");
    let facts = rig.hub.facts_of(pane);
    let mut chips = lead_chips(&folder, &facts);
    chips.extend(chip_texts(&facts));
    let said: Vec<(ChipKind, String)> = chips.iter().map(|c| (c.kind, c.text())).collect();
    assert_eq!(
        said,
        [
            (ChipKind::Folder, "~/app".to_owned()),
            (ChipKind::Branch, "main".to_owned()),
            (ChipKind::Changes, "+1 \u{2212}0 \u{b7} 1 file".to_owned()),
        ],
        "the host's chips"
    );
}
