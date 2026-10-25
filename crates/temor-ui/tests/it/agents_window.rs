//! Agents in the window, in ds-harness (event dispatch only, never OS input): a program that
//! says it waits (OSC 7501, or over the pane socket) puts the sidebar's one dot on its tab, and
//! ⌘⇧U brings the keyboard to the pane that waits.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ds::base::vocab::ShortcutKey;

use ds_harness::{Harness, Input};
use temor_pane::report::{ReportState, StatusLine, encode_hello, encode_status};
use temor_ui::view::probe::Probe;

use super::drive::{chord, focus_pane, started, type_line, wait_until};
use super::support::{scratch_path, tools_present};

/// How many tab rows show the dot that says the tab needs the person.
fn dots(h: &Harness) -> usize {
    h.html().matches("class=\"temor-tab-needs\"").count()
}

fn wait_for_dots(h: &mut Harness, what: &str, want: usize) {
    let start = Instant::now();
    while dots(h) != want {
        h.advance(Duration::from_millis(25));
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "timed out waiting for {what}:\n{}",
            h.html()
        );
    }
}

/// The pane sockets in the window's runtime folder, oldest pane first.
fn sockets(name: &str) -> Vec<PathBuf> {
    let dir = scratch_path(name).join("home/runtime");
    let mut found: Vec<(u64, PathBuf)> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter_map(|p| {
                    let id = p.file_stem()?.to_str()?.parse().ok()?;
                    (p.extension()? == "sock").then_some((id, p))
                })
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found.into_iter().map(|(_, p)| p).collect()
}

fn say_over_socket(socket: &Path, state: ReportState) {
    let stream = UnixStream::connect(socket).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("timeout");
    let mut reader = BufReader::new(stream.try_clone().expect("clone"));
    let mut writer = stream;
    let mut reply = String::new();
    writer.write_all(encode_hello().as_bytes()).expect("hello");
    reader.read_line(&mut reply).expect("hello reply");
    let line = StatusLine {
        seq: 1,
        state,
        wait: None,
        app: Some("claude".to_owned()),
        note: None,
    };
    writer
        .write_all(encode_status(&line).as_bytes())
        .expect("status");
    reply.clear();
    reader.read_line(&mut reply).expect("status reply");
    assert_eq!(reply.trim(), "ok 1");
}

#[test]
fn a_wait_shows_as_the_dot_and_the_jump_chord_finds_the_pane_that_waits() {
    if !tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("ag-dot", &probe);
    focus_pane(&mut h);
    wait_for_dots(&mut h, "no dot", 0);

    // The program in the first tab says it needs permission, in its own status sequence, and
    // then waits for an answer as an agent does (a prompt coming back would end the wait).
    type_line(
        &mut h,
        "printf '\\033]7501;state=blocked:kind=permission:app=claude\\007'; read answer",
    );
    wait_for_dots(&mut h, "the waiting tab's dot", 1);

    // A second tab, and its program reports over the pane's socket.
    h.send(Input::chord(
        &[ShortcutKey::Ctrl, ShortcutKey::Shift],
        ShortcutKey::Char('n'),
    ));
    h.advance(Duration::from_millis(300));
    type_line(&mut h, "echo second-tab");
    wait_until(&mut h, &probe, "the second tab to have the keys", |s| {
        s.lines().any(|l| l.trim_end() == "second-tab")
    });
    let found = sockets("ag-dot");
    assert_eq!(found.len(), 2, "one socket for each pane: {found:?}");
    // Working is not something the row says: the dots stay as they were.
    say_over_socket(&found[1], ReportState::Working);
    h.advance(Duration::from_millis(300));
    assert_eq!(dots(&h), 1, "a working agent puts no dot on its tab");

    // The chord brings the keyboard to the pane that waits: its screen has the command typed.
    h.send(Input::chord(
        &[ShortcutKey::Super, ShortcutKey::Shift],
        ShortcutKey::Char('u'),
    ));
    wait_until(&mut h, &probe, "the waiting pane to have the keys", |s| {
        s.contains("7501")
    });
}

/// A tab split in two draws its panes as segments; the pane whose program says it waits still
/// puts the dot on its tab.
#[test]
fn a_waiting_program_in_a_split_tab_still_shows_its_dot() {
    if !tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("ag-split-dot", &probe);
    focus_pane(&mut h);
    // ⌘D: the tab splits, and the new pane has the keys.
    chord(&mut h, 'd');
    assert_eq!(
        h.count("[aria-label=\"Terminal\"]"),
        2,
        "the tab did not split"
    );
    type_line(
        &mut h,
        "printf '\\033]7501;state=blocked:kind=permission:app=claude\\007'; read answer",
    );
    wait_for_dots(&mut h, "the split tab's dot", 1);
}
