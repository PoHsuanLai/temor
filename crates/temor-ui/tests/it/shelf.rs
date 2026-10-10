//! Transfers and the Downloads shelf in the window, end to end in ds-harness: a program in the
//! shell prints a file through the terminal, the window asks, and only after "Allow" does the
//! file reach the Downloads folder and the shelf (event dispatch only, never OS input).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use ds::base::vocab::ShortcutKey;
use ds::prelude::{Point, Px};
use ds_harness::{Harness, Input};
use temor_ui::view::probe::Probe;

use super::window::{harness, wait_until};

fn type_line(h: &mut Harness, text: &str) {
    for c in text.chars() {
        let key = match c {
            ' ' => ShortcutKey::Space,
            c => ShortcutKey::Char(c),
        };
        h.send(Input::key(key));
    }
    h.send(Input::key(ShortcutKey::Enter));
}

fn wait_for_html(h: &mut Harness, what: &str, ok: impl Fn(&str) -> bool) {
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

#[test]
fn a_file_a_program_prints_waits_for_allow_then_lands_in_downloads_and_on_the_shelf() {
    if !super::support::tools_present() {
        return;
    }
    let name = "shelf-window";
    let home = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(name)
        .join("home");
    let probe = Probe::new();
    let mut h = harness(name, &probe);
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    h.send(Input::click(Point {
        x: Px(300.0),
        y: Px(200.0),
    }));
    h.advance(Duration::from_millis(100));
    // "note.txt" holding "hello", as iTerm2's File= sends it.
    type_line(
        &mut h,
        "printf '\\033]1337;File=name=bm90ZS50eHQ=;size=5:aGVsbG8=\\033\\\\'",
    );

    wait_for_html(&mut h, "the question", |html| {
        html.contains("to your Downloads folder")
    });
    assert!(
        h.html().contains("note.txt"),
        "the file is named in the question"
    );
    assert!(
        !home.join("Downloads").join("note.txt").exists(),
        "nothing is saved before the person says yes"
    );

    let allow = h
        .centre(".ds-inline-banner .ds-button")
        .unwrap_or_else(|| panic!("no Allow button:\n{}", h.html()));
    h.send(Input::click(allow));

    let saved = home.join("Downloads").join("note.txt");
    let start = Instant::now();
    while !saved.exists() {
        h.advance(Duration::from_millis(25));
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "the file never arrived:\n{}",
            h.html()
        );
    }
    assert_eq!(std::fs::read(&saved).expect("saved"), b"hello");
    wait_for_html(&mut h, "the shelf's badge", |html| {
        html.contains("Downloads (1)") && !html.contains("to your Downloads folder")
    });

    // Opening the shelf shows the file as a row.
    let trigger = h
        .centre(".temor-dl-box .ds-disclosure-trigger")
        .unwrap_or_else(|| panic!("no Downloads disclosure:\n{}", h.html()));
    h.send(Input::click(trigger));
    wait_for_html(&mut h, "the shelf's rows", |html| {
        html.contains("temor-dl") && html.contains("note.txt") && html.contains("Show in folder")
    });
}
