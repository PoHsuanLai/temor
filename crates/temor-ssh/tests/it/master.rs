//! The shared connection's directories and probe, in a scratch directory with a stand-in `ssh`.

use std::fs;
use std::os::unix::fs::PermissionsExt;

use temor_ssh::{
    ControlLayout, ControlPersist, FileTools, HostAlias, MasterProbe, RuntimeDir,
    prepare_control_dirs, probe_master,
};

use crate::harness::{RetryRunner, make_bin, scratch};

fn layout_in(root: &std::path::Path) -> ControlLayout {
    ControlLayout::new(
        RuntimeDir::new(&root.join("temor")).expect("runtime"),
        ControlPersist::Forever,
    )
    .expect("layout")
}

#[test]
fn the_socket_directories_are_made_private_and_made_again_harmlessly() {
    let s = scratch();
    let layout = layout_in(&s.work);
    prepare_control_dirs(&layout).expect("first");
    prepare_control_dirs(&layout).expect("second");
    for dir in [layout.runtime_dir().to_path_buf(), layout.socket_dir()] {
        let mode = fs::metadata(&dir).expect("dir").permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "{}", dir.display());
    }
}

#[test]
fn a_probe_reads_the_exit_of_ssh_dash_o_check() {
    let s = scratch();
    let alias = HostAlias::new("prod").expect("alias");
    let layout = layout_in(&s.work);
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
fn a_directory_of_ours_with_no_read_permission_is_tightened_not_refused() {
    let s = scratch();
    let layout = layout_in(&s.work);
    prepare_control_dirs(&layout).expect("first");
    let socket_dir = layout.socket_dir();
    for mode in [0o300, 0o000, 0o755] {
        fs::set_permissions(&socket_dir, fs::Permissions::from_mode(mode)).expect("chmod");
        prepare_control_dirs(&layout).unwrap_or_else(|err| panic!("{mode:o}: {err}"));
        let now = fs::metadata(&socket_dir).expect("dir").permissions().mode() & 0o777;
        assert_eq!(now, 0o700, "from {mode:o}");
    }
}
