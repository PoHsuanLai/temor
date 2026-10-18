//! A host pane in the window (ds-harness, event dispatch only). The pane is restored from a
//! scratch session file as a session on `fake`, whose `ssh` is a stand-in on a private `PATH`:
//! the questions come one at a time, and the browser check shows a row without opening anything.
//! Nothing here reaches a network, the real `~/.ssh` or the desktop's opener.

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use ds_harness::{Harness, Input};
use temor_core::host::consent::{BootstrapGrants, ConsentBook, HostRecord};
use temor_core::host::store::{HostsFile, PaneHost};
use temor_core::remote::Grant;
use temor_core::vocab::ids::PaneId;
use temor_core::workspace::{ClosedState, KeepClosed, Workspace, WorkspaceIn, workspace_step};
use temor_ui::edge::save_with_hosts;
use temor_ui::view::probe::Probe;

use super::support::{ScratchRoot, socket_root, tools_present};
use super::window::{harness_with, wait_until};

/// temor's runtime folder below the scratch root.
const RUNTIME: &str = "xdg/temor";

/// Scratch folders with a stand-in `ssh` running `body`, and a session file with one tab whose
/// pane is a session on `fake`, with `book` as the remembered answers. The root is short because
/// a shared connection's socket path has to fit a unix socket.
fn setup(name: &str, body: &str, book: ConsentBook) -> (ScratchRoot, PathBuf) {
    let root = socket_root(name, RUNTIME);
    let bin = root.join("bin");
    std::fs::create_dir_all(&bin).expect("bin folder");
    std::fs::create_dir_all(root.join("xdg")).expect("runtime folder");
    let ssh = bin.join("ssh");
    std::fs::write(&ssh, body).expect("stand-in ssh");
    std::fs::set_permissions(&ssh, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let ws = workspace_step(
        Workspace::new(ClosedState::new(KeepClosed::DEFAULT)),
        WorkspaceIn::NewTab { cwd: None },
    )
    .0;
    let state = root.join("state");
    let hosts = HostsFile {
        book,
        panes: vec![PaneHost {
            pane: PaneId(1),
            alias: "fake".to_owned(),
            session: None,
        }],
    };
    save_with_hosts(&state, &ws, &hosts).expect("session saved");
    (root, state)
}

fn window(name: &str, probe: &Probe, root: &std::path::Path, state: PathBuf) -> Harness {
    let (bin, runtime) = (root.join("bin"), root.join(RUNTIME));
    harness_with(name, probe, Some(state), move |ambient| {
        ambient
            .env
            .set("PATH", format!("{}:/usr/bin:/bin", bin.display()));
        ambient.runtime_dir = runtime;
    })
}

fn wait_html(h: &mut Harness, what: &str, ok: impl Fn(&str) -> bool) {
    let start = Instant::now();
    while !ok(&h.html()) {
        h.advance(Duration::from_millis(25));
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "timed out waiting for {what}:\n{}",
            h.html()
        );
    }
}

fn press(h: &mut Harness, selector: &str) {
    let at = h
        .centre(selector)
        .unwrap_or_else(|| panic!("no {selector}:\n{}", h.html()));
    h.send(Input::click(at));
    h.advance(Duration::from_millis(100));
}

#[test]
fn the_questions_before_a_first_connection_come_one_at_a_time_and_then_ssh_starts() {
    if !tools_present() {
        return;
    }
    let script = "#!/bin/sh\n[ \"$1\" = -O ] && exit 255\necho FAKE-SSH-SESSION\nexec sleep 60\n";
    let (root, state) = setup("ssh-asks", script, ConsentBook::default());
    let probe = Probe::new();
    let mut h = window("ssh-asks", &probe, &root, state);

    wait_html(&mut h, "the first question", |html| {
        html.contains("Plain connection")
    });
    assert!(!h.html().contains("Add it"), "one question at a time");
    press(
        &mut h,
        ".temor-host .ds-inline-banner .ds-button:first-of-type",
    );

    wait_html(&mut h, "the second question", |html| {
        html.contains("Add it")
    });
    assert!(
        !h.html().contains("Plain connection"),
        "the first is answered"
    );
    press(
        &mut h,
        ".temor-host .ds-inline-banner .ds-button:last-of-type",
    );

    wait_until(&mut h, &probe, "the session", |s| {
        s.contains("FAKE-SSH-SESSION")
    });
    assert!(!h.html().contains("Add it"));
}

#[test]
fn the_browser_check_shows_a_row_and_opens_nothing_by_itself() {
    if !tools_present() {
        return;
    }
    let script = "#!/bin/sh\n[ \"$1\" = -O ] && exit 255\n\
        echo 'To authenticate, visit: https://login.tailscale.com/a/abc123'\nexec sleep 60\n";
    let answered = ConsentBook::from_records(vec![HostRecord {
        aliases: vec!["fake".to_owned()],
        grants: BootstrapGrants {
            integrate: Grant::Allowed,
            terminfo: Grant::Refused,
            ..BootstrapGrants::default()
        },
        ..HostRecord::default()
    }]);
    let (root, state) = setup("ssh-check", script, answered);
    let probe = Probe::new();
    let mut h = window("ssh-check", &probe, &root, state);

    wait_html(&mut h, "the check row", |html| {
        html.contains("in your browser")
    });
    assert!(!h.html().contains("Plain connection"), "nothing is asked");
}
