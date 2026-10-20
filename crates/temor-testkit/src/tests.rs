use std::time::Duration;

use super::*;

#[test]
fn a_program_on_the_path_is_found_and_one_that_is_not_is_not() {
    assert!(find_program("sh").is_some());
    assert_eq!(find_program("temor-no-such-program-anywhere"), None);
}

#[test]
fn an_optional_program_that_is_missing_skips_whatever_the_gate_says() {
    assert_eq!(
        optional_program("temor-no-such-program-anywhere", "t"),
        None
    );
}

#[test]
fn polling_returns_the_value_once_it_comes() {
    let mut calls = 0;
    let got = eventually_some("a third call", Duration::from_secs(5), || {
        calls += 1;
        (calls == 3).then_some(calls)
    });
    assert_eq!(got, 3);
}

#[test]
#[should_panic(expected = "timed out waiting for never")]
fn polling_gives_up_naming_what_it_waited_for() {
    eventually("never", Duration::from_millis(30), || false);
}

#[test]
fn a_pid_file_is_read_once_it_holds_a_number() {
    let scratch = Scratch::new();
    let file = scratch.root.join("pid");
    std::fs::write(&file, "4242\n").expect("write");
    assert_eq!(pid_from_file(&file, Duration::from_secs(1)), 4242);
}

#[test]
fn byte_search_finds_the_first_match_and_an_empty_needle_at_the_start() {
    assert_eq!(find(b"abcabc", b"ca"), Some(2));
    assert_eq!(find(b"abc", b"x"), None);
    assert_eq!(find(b"abc", b""), Some(0));
    assert!(contains(b"hello world", "o w"));
}

#[test]
fn a_scratch_points_home_and_every_xdg_folder_inside_itself() {
    let scratch = Scratch::new();
    for (name, value) in scratch.env() {
        let path = std::path::PathBuf::from(value);
        assert!(path.starts_with(&scratch.root), "{name}");
        assert!(path.is_dir(), "{name}");
    }
}

#[test]
fn a_socket_root_leaves_room_for_the_longest_socket_and_goes_when_dropped() {
    let root = ScratchRoot::for_sockets("kit", 60);
    assert!(root.as_os_str().len() + 60 <= SOCKET_PATH_MAX);
    let path = root.to_path_buf();
    assert!(path.is_dir());
    drop(root);
    assert!(!path.exists());
}

#[test]
fn git_makes_a_repository_in_a_scratch_folder() {
    if program_or_skip("git", "git_makes_a_repository_in_a_scratch_folder").is_none() {
        return;
    }
    let scratch = Scratch::new();
    git(&scratch.root, &["init", "-q"]);
    assert!(scratch.root.join(".git").is_dir());
}
