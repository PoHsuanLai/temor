//! `Include` paths, budgets and the real filesystem reader's refusals.

use super::model::IssueReason;
use super::support::{aliases, load, one};

#[test]
fn includes_resolve_relative_to_ssh_dir_and_globs_in_order() {
    let list = load(&[
        (
            "config",
            "Include conf.d/*.conf\nInclude missing-file\nHost top\n",
        ),
        ("conf.d/b.conf", "Host second\n  User b\n"),
        ("conf.d/a.conf", "Host first\n  User a\n"),
        ("conf.d/ignored.txt", "Host nope\n"),
    ]);
    assert!(list.issues.is_empty(), "{:?}", list.issues);
    assert_eq!(aliases(&list), vec!["first", "second", "top"]);
    assert_eq!(
        one(&list, "second")
            .user
            .map(|u| u.as_str().to_owned())
            .as_deref(),
        Some("b")
    );
}

#[test]
fn include_cycles_stop_with_an_issue() {
    let list = load(&[("config", "Include config\nHost loop\n")]);
    assert!(
        list.issues
            .iter()
            .any(|i| i.reason == IssueReason::IncludeTooDeep)
    );
    assert_eq!(aliases(&list), vec!["loop"]);
}

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

use super::fs::MAX_FILE_BYTES;
use super::{ConfigPaths, FsConfigFiles, HostList, load_host_list};

/// Loads the config in `dir` from the real filesystem. The load runs on its own thread, so a
/// hang fails the test instead of stalling it.
fn load_real(dir: &Path) -> HostList {
    let paths = ConfigPaths {
        config: dir.join("config"),
        ssh_dir: dir.to_path_buf(),
        home: None,
    };
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(load_host_list(&FsConfigFiles, &paths));
    });
    rx.recv_timeout(Duration::from_secs(10))
        .expect("loading the config hung")
}

fn write(path: &Path, text: &str) {
    std::fs::write(path, text).expect("write fixture");
}

fn has_issue(list: &HostList, reason: &IssueReason) -> bool {
    list.issues.iter().any(|issue| &issue.reason == reason)
}

fn fifo_in(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    let status = Command::new("mkfifo").arg(&path).status().expect("mkfifo");
    assert!(status.success(), "mkfifo failed");
    path
}

#[test]
fn a_fifo_include_is_an_issue_and_never_blocks() {
    let dir = tempfile::tempdir().expect("dir");
    fifo_in(dir.path(), "pipe.conf");
    write(
        &dir.path().join("config"),
        "Include pipe.conf\nHost after\n",
    );
    let list = load_real(dir.path());
    assert!(
        list.issues
            .iter()
            .any(|i| matches!(i.reason, IssueReason::IncludeUnreadable { .. })),
        "{:?}",
        list.issues
    );
    assert_eq!(aliases(&list), vec!["after"]);
}

#[test]
fn a_device_include_is_an_issue_and_never_streams() {
    let dir = tempfile::tempdir().expect("dir");
    write(
        &dir.path().join("config"),
        "Include /dev/zero\nHost after\n",
    );
    let list = load_real(dir.path());
    assert!(
        list.issues
            .iter()
            .any(|i| matches!(i.reason, IssueReason::IncludeUnreadable { .. })),
        "{:?}",
        list.issues
    );
    assert_eq!(aliases(&list), vec!["after"]);
}

#[test]
fn a_self_including_glob_hits_the_read_budget() {
    let dir = tempfile::tempdir().expect("dir");
    write(&dir.path().join("config"), "Include *.conf\n");
    write(&dir.path().join("one.conf"), "Include *.conf\nHost one\n");
    write(&dir.path().join("two.conf"), "Include *.conf\nHost two\n");
    let list = load_real(dir.path());
    assert!(
        has_issue(&list, &IssueReason::IncludeBudgetExceeded),
        "{:?}",
        list.issues
    );
    let mut names = aliases(&list);
    names.sort_unstable();
    assert_eq!(names, vec!["one", "two"]);
}

#[test]
fn many_small_files_hit_the_read_budget_once_reported() {
    let dir = tempfile::tempdir().expect("dir");
    write(&dir.path().join("config"), "Include *.conf\n");
    for n in 0..300 {
        write(
            &dir.path().join(format!("h{n:03}.conf")),
            &format!("Host h{n:03}\n"),
        );
    }
    let list = load_real(dir.path());
    let budget = list
        .issues
        .iter()
        .filter(|i| i.reason == IssueReason::IncludeBudgetExceeded)
        .count();
    assert_eq!(budget, 1, "{:?}", list.issues);
    assert_eq!(
        list.entries.len(),
        255,
        "256 reads include the config itself"
    );
}

#[test]
fn the_byte_budget_stops_large_includes() {
    let dir = tempfile::tempdir().expect("dir");
    write(&dir.path().join("config"), "Include *.conf\n");
    // Each file is under the per-file cap; five of them pass the 1 MiB total.
    let filler = "#".repeat(usize::try_from(MAX_FILE_BYTES - 64).unwrap_or(0));
    for n in 0..5 {
        write(
            &dir.path().join(format!("big{n}.conf")),
            &format!("{filler}\nHost big{n}\n"),
        );
    }
    let list = load_real(dir.path());
    assert!(
        has_issue(&list, &IssueReason::IncludeBudgetExceeded),
        "{:?}",
        list.issues
    );
    assert!(aliases(&list).len() < 5);
}

#[test]
fn a_file_over_the_per_file_cap_is_unreadable() {
    let dir = tempfile::tempdir().expect("dir");
    let big = "#".repeat(usize::try_from(MAX_FILE_BYTES + 10).unwrap_or(0));
    write(&dir.path().join("config"), "Include big.conf\nHost after\n");
    write(&dir.path().join("big.conf"), &big);
    let list = load_real(dir.path());
    assert!(
        list.issues
            .iter()
            .any(|i| matches!(i.reason, IssueReason::IncludeUnreadable { .. })),
        "{:?}",
        list.issues
    );
    assert_eq!(aliases(&list), vec!["after"]);
}
