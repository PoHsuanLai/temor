//! Transfers and the Downloads shelf in the window, end to end in ds-harness: a program in the
//! shell prints a file through the terminal, the window asks, and only after "Allow" does the
//! file reach the Downloads folder and the shelf (event dispatch only, never OS input).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use ds::base::press::PointerButton;
use ds::prelude::Point;
use ds_harness::{Harness, Input};
use temor_ui::view::probe::Probe;

use super::drive::{focus_pane, type_line, wait_html, wait_until};
use super::window::harness;

/// Where `selector` is drawn once the list has finished opening: the same over two frames, and a
/// press there lands on it. Clicking earlier can land on whatever was under the list.
fn settled_centre(h: &mut Harness, selector: &str) -> Point {
    let start = Instant::now();
    loop {
        let before = h.centre(selector);
        h.advance(Duration::from_millis(50));
        if let Some(at) = h.centre(selector)
            && before == Some(at)
            && h.hits(at, selector)
        {
            return at;
        }
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "{selector} never settled:\n{}",
            h.html()
        );
    }
}

#[test]
fn a_file_a_program_prints_waits_for_allow_then_lands_in_downloads_and_on_the_shelf() {
    if !super::support::window_tools_present() {
        return;
    }
    let name = "shelf-window";
    let home = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(name)
        .join("home");
    let probe = Probe::new();
    let mut h = harness(name, &probe);
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    focus_pane(&mut h);
    // "note.txt" holding "hello", as iTerm2's File= sends it.
    type_line(
        &mut h,
        "printf '\\033]1337;File=name=bm90ZS50eHQ=;size=5:aGVsbG8=\\033\\\\'",
    );

    wait_html(&mut h, "the question", |html| {
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
    // The Downloads button in the head carries a dot: a file arrived and is not yet opened.
    wait_html(&mut h, "the shelf's dot", |html| {
        html.contains("class=\"temor-dl-dot\"") && !html.contains("to your Downloads folder")
    });

    // Opening the shelf shows the file as a row.
    let trigger = h
        .centre(".temor-dl-hold .ds-button")
        .unwrap_or_else(|| panic!("no Downloads button:\n{}", h.html()));
    h.send(Input::click(trigger));
    wait_html(&mut h, "the shelf's rows", |html| {
        html.contains("temor-dl") && html.contains("note.txt") && html.contains("Show in Folder")
    });

    // Paste path types the quoted path into the terminal the person was using and puts the list
    // away; nothing runs.
    let paste = settled_centre(
        &mut h,
        ".temor-dl-actions .ds-button[aria-description=\"Type the file's path into the terminal you were using\"]",
    );
    h.send(Input::click(paste));
    wait_until(&mut h, &probe, "the pasted path", |s| {
        s.replace('\n', "").contains("Downloads/note.txt")
    });
    assert!(
        !h.html().contains("Show in Folder"),
        "the list is put away:\n{}",
        h.html()
    );

    // A secondary press on the row opens every action as a menu.
    let trigger = h
        .centre(".temor-dl-hold .ds-button")
        .expect("Downloads button");
    h.send(Input::click(trigger));
    wait_html(&mut h, "the shelf's rows", |html| {
        html.contains("Show in Folder")
    });
    let row = settled_centre(&mut h, ".temor-dl");
    h.send(Input::press(row, PointerButton::Secondary));
    wait_html(&mut h, "the row's menu", |html| {
        html.contains("ds-menu-item")
    });
    let html = h.html();
    for item in [
        "Open",
        "Show in Folder",
        "Paste Path",
        "Copy Path",
        "Remove from List",
    ] {
        assert!(html.contains(item), "the menu has {item}:\n{html}");
    }
}
