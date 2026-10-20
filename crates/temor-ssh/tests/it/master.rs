//! The shared connection's directories and probe, in a scratch directory with a stand-in `ssh`.

use std::fs;
use std::os::unix::fs::PermissionsExt;

use temor_ssh::{
    ControlLayout, ControlPersist, FileTools, HostAlias, MasterProbe, RuntimeDir,
    prepare_control_dirs, probe_master, socket_appeared,
};

use crate::harness::{RetryRunner, make_bin, scratch};

/// A layout under a short folder of its own: a socket path must fit `sun_path`, and the system's
/// temp folder alone takes most of macOS's 104 bytes. Keep the folder while the layout is used.
fn layout() -> (tempfile::TempDir, ControlLayout) {
    let root = tempfile::Builder::new()
        .prefix("tm")
        .tempdir_in("/tmp")
        .expect("short root");
    let layout = ControlLayout::new(
        RuntimeDir::new(&root.path().join("temor")).expect("runtime"),
        ControlPersist::Forever,
    )
    .expect("layout");
    (root, layout)
}

#[test]
#[cfg_attr(
    not(any(target_os = "linux", target_os = "macos")),
    ignore = "the private runtime folder is Linux-only so far"
)]
fn the_socket_directories_are_made_private_and_made_again_harmlessly() {
    let (_root, layout) = layout();
    prepare_control_dirs(&layout).expect("first");
    prepare_control_dirs(&layout).expect("second");
    for dir in [layout.runtime_dir().to_path_buf(), layout.socket_dir()] {
        let mode = fs::metadata(&dir).expect("dir").permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "{}", dir.display());
    }
}

#[test]
#[cfg_attr(
    not(any(target_os = "linux", target_os = "macos")),
    ignore = "the private runtime folder is Linux-only so far"
)]
fn a_master_is_ready_when_a_finished_socket_appears_not_its_temporary_name() {
    let (_root, layout) = layout();
    prepare_control_dirs(&layout).expect("dirs");
    let old = "a".repeat(40);
    let new = "b".repeat(40);
    fs::write(layout.socket_dir().join(&old), "").expect("an older master");
    let before = layout.finished_sockets();
    assert_eq!(before.len(), 1);

    // Name, what is in the folder after, whether the new master shows.
    let table = [
        ("nothing new", vec![], false),
        (
            "the temporary name ssh listens under first",
            vec![format!("{new}.AbCdEfGhIjKlMnOp")],
            false,
        ),
        ("renamed into place", vec![new.clone()], true),
    ];
    for (name, add, want) in table {
        for file in &add {
            fs::write(layout.socket_dir().join(file), "").expect("socket");
        }
        assert_eq!(
            socket_appeared(&before, &layout.finished_sockets()),
            want,
            "{name}"
        );
    }
}

#[test]
fn a_probe_reads_the_exit_of_ssh_dash_o_check() {
    let s = scratch();
    let alias = HostAlias::new("prod").expect("alias");
    let (_root, layout) = layout();
    // Stand-ins that answer like ssh does to `-O check`, and refuse anything else (exit 64).
    let guard = "[ \"$1 $2\" = '-O check' ] || exit 64\n";
    for (name, exit, want) in [
        ("up", 0, MasterProbe::Running),
        ("socket missing", 255, MasterProbe::NotRunning),
    ] {
        let dir = s.bin.join(name.replace(' ', "-"));
        fs::create_dir_all(&dir).expect("dir");
        let body = format!("#!/bin/sh\n{guard}exit {exit}\n");
        make_bin(&dir, &[], &[("ssh", body.as_str())]);
        let tools = FileTools {
            ssh: dir.join("ssh"),
            scp: dir.join("scp"),
        };
        let got = probe_master(&RetryRunner, &tools, &alias, &layout).expect("probe");
        assert_eq!(got, want, "{name}");
    }
}

#[test]
#[cfg_attr(
    not(any(target_os = "linux", target_os = "macos")),
    ignore = "the private runtime folder is Linux-only so far"
)]
fn a_directory_of_ours_with_no_read_permission_is_tightened_not_refused() {
    let (_root, layout) = layout();
    prepare_control_dirs(&layout).expect("first");
    let socket_dir = layout.socket_dir();
    for mode in [0o300, 0o000, 0o755] {
        fs::set_permissions(&socket_dir, fs::Permissions::from_mode(mode)).expect("chmod");
        prepare_control_dirs(&layout).unwrap_or_else(|err| panic!("{mode:o}: {err}"));
        let now = fs::metadata(&socket_dir).expect("dir").permissions().mode() & 0o777;
        assert_eq!(now, 0o700, "from {mode:o}");
    }
}
