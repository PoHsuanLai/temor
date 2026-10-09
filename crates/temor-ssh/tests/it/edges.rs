//! Bootstrap edge cases run by real shells: UTF-8 directories, the umask the person's shell
//! inherits, stale temp directories, and a TMPDIR that cannot be used.

use std::fs;
use std::os::unix::fs::symlink;
use std::process::Command;

use crate::harness::{Launch, contains, host_name, require, scratch, write};

#[test]
fn utf8_cwd_is_percent_encoded_under_dash() {
    let Some(dash) = require("dash") else {
        return;
    };
    let s = scratch();
    let work = s.work.join("日本").join("é");
    fs::create_dir_all(&work).expect("utf-8 dir");
    let mut launch = Launch::new(&s);
    launch.outer = dash.clone();
    launch.shell = dash.display().to_string();
    launch.cwd = work;
    let ran = launch.run();
    assert!(ran.status.success(), "{:?}", ran.status);
    // The scratch path is ASCII, so it appears as is before the encoded UTF-8 components.
    let osc = format!(
        "\x1b]7;file://{}{}/%E6%97%A5%E6%9C%AC/%C3%A9\x1b\\",
        host_name(),
        s.work.display()
    );
    assert!(
        contains(&ran.text, &osc),
        "no exact OSC 7 under dash: {:?}",
        ran.text
    );
}

#[test]
fn the_login_shell_starts_with_the_umask_the_person_had() {
    let s = scratch();
    let mut launch = Launch::new(&s);
    // The outer shell runs with umask 027; the bootstrap sets 077 while it works, then restores 027.
    launch.command = format!("umask 027; {}", launch.command);
    launch.input = "umask\nexit\n".to_owned();
    let ran = launch.run();
    assert!(ran.status.success(), "{:?}", ran.status);
    assert!(
        contains(&ran.text, "0027\n"),
        "umask not restored: {:?}",
        ran.text
    );
}

#[test]
fn stale_temp_dirs_of_dead_sessions_are_removed_and_nothing_else() {
    let s = scratch();
    // A pid that has exited: spawn a child and reap it.
    let mut child = Command::new("true").spawn().expect("true");
    let dead = child.id().to_string();
    child.wait().expect("reap");

    let dead_dir = s.tmp.join("temor.dead");
    fs::create_dir(&dead_dir).expect("dead dir");
    write(&dead_dir.join("pid"), &format!("{dead}\n"));

    let live = s.tmp.join("temor.live");
    fs::create_dir(&live).expect("live dir");
    write(&live.join("pid"), &format!("{}\n", std::process::id()));

    let nopid = s.tmp.join("temor.nopid");
    fs::create_dir(&nopid).expect("nopid dir");
    let junk = s.tmp.join("temor.junk");
    fs::create_dir(&junk).expect("junk dir");
    write(&junk.join("pid"), "not-a-pid\n");

    let file = s.tmp.join("temor.file");
    write(&file, &dead);

    let target = s.work.join("target");
    fs::create_dir(&target).expect("symlink target");
    write(&target.join("pid"), &format!("{dead}\n"));
    let link = s.tmp.join("temor.link");
    symlink(&target, &link).expect("symlink");

    let other = s.tmp.join("other.dead");
    fs::create_dir(&other).expect("other dir");
    write(&other.join("pid"), &format!("{dead}\n"));

    let ran = Launch::new(&s).run();
    assert!(ran.status.success(), "{:?}", ran.status);

    assert!(!dead_dir.exists(), "dead session dir kept");
    assert!(live.is_dir(), "live session dir removed");
    assert!(nopid.is_dir(), "dir without a pid removed");
    assert!(junk.is_dir(), "dir with a bad pid removed");
    assert!(file.is_file(), "a plain file was removed");
    assert!(link.symlink_metadata().is_ok(), "the symlink was removed");
    assert!(
        target.is_dir(),
        "the symlink target was followed and removed"
    );
    assert!(other.is_dir(), "a dir outside temor.* was removed");
}

#[test]
fn a_tmpdir_with_a_colon_is_refused_with_a_clear_message() {
    let s = scratch();
    let colon = s.tmp.join("a:b");
    fs::create_dir(&colon).expect("colon dir");
    let mut launch = Launch::new(&s);
    launch.tmp = colon.clone();
    let ran = launch.run();
    let text = String::from_utf8_lossy(&ran.text).into_owned();
    assert_eq!(ran.status.code(), Some(1), "{text}");
    assert!(text.contains("TMPDIR must not contain ':'"), "{text}");
    assert_eq!(
        fs::read_dir(&colon).expect("dir").count(),
        0,
        "files written"
    );
}
