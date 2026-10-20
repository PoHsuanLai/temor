//! Agents in the window, in ds-harness (event dispatch only, never OS input): a program's own
//! status report (OSC 7501) and a report over the pane socket show as the sidebar's orbs, and
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
use super::window::harness;

/// How many tab rows show `presence` (`idle`, `working`, `waiting`). An idle tab has no orb (a
/// terminal glyph leads it): the tabs that show neither of the others.
fn orbs(h: &Harness, presence: &str) -> usize {
    let html = h.html();
    let count = |presence: &str| {
        html.matches(&format!("data-presence=\"{presence}\""))
            .count()
    };
    match presence {
        "idle" => {
            html.matches("class=\"temor-tab-box\"").count() - count("working") - count("waiting")
        }
        _ => count(presence),
    }
}

fn wait_for_orbs(h: &mut Harness, what: &str, want: &[(&str, usize)]) {
    let start = Instant::now();
    while !want.iter().all(|(presence, n)| orbs(h, presence) == *n) {
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
fn reports_show_as_orbs_and_the_jump_chord_finds_the_pane_that_waits() {
    if !tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("ag-orb", &probe);
    focus_pane(&mut h);
    wait_for_orbs(&mut h, "an idle orb", &[("idle", 1)]);

    // The program in the first tab says it needs permission, in its own status sequence, and
    // then waits for an answer as an agent does (a prompt coming back would end the wait).
    type_line(
        &mut h,
        "printf '\\033]7501;state=blocked:kind=permission:app=claude\\007'; read answer",
    );
    wait_for_orbs(&mut h, "a waiting orb", &[("waiting", 1), ("idle", 0)]);

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
    let found = sockets("ag-orb");
    assert_eq!(found.len(), 2, "one socket for each pane: {found:?}");
    say_over_socket(&found[1], ReportState::Working);
    wait_for_orbs(&mut h, "a working orb", &[("waiting", 1), ("working", 1)]);

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
/// shows as a waiting orb in the sidebar (found by the end-to-end workflow test: a split tab's
/// row draws each segment's prompt glyph and drops the tab's orb).
#[test]
fn a_waiting_program_in_a_split_tab_still_shows_its_orb() {
    if !tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("ag-split-orb", &probe);
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
    wait_for_orbs(&mut h, "a waiting orb on the split tab", &[("waiting", 1)]);
}
