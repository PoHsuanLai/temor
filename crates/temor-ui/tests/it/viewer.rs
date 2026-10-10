//! A viewer pane in the window (PLAN §10): ⌘-click on a file the shell listed opens a viewer
//! beside the terminal, ⌘W closes it, and Esc gives the keyboard back to the terminal. Event
//! dispatch only, as the rest of the window tests; the viewer reads real files in a scratch
//! folder.

use std::time::{Duration, Instant};

use dioxus::html::Modifiers;
use ds::base::press::PointerButton;
use ds::base::vocab::ShortcutKey;
use ds::prelude::{Point, Px};
use ds_harness::{Driver, Harness, Input, PointerAction, PointerInput};
use temor_ui::view::probe::Probe;

use super::support::{scratch_path, tools_present};
use super::window::{chord, harness, wait_until};

fn viewers(h: &Harness) -> usize {
    h.html().matches("aria-label=\"File viewer\"").count()
}

fn terminals(h: &Harness) -> usize {
    h.html().matches("aria-label=\"Terminal\"").count()
}

/// A ⌘-click: on our desktop that is Ctrl+Shift held, as the window's keymap reads it.
fn command_click(h: &mut Harness, at: Point) {
    let held = Modifiers::CONTROL | Modifiers::SHIFT;
    let click = PointerInput::new(at, PointerAction::Click(PointerButton::Primary)).with_mods(held);
    h.send(Input::Pointer(click));
    h.advance(Duration::from_millis(60));
}

fn type_line(h: &mut Harness, line: &str) {
    for c in line.chars() {
        let key = match c {
            ' ' => ShortcutKey::Space,
            c => ShortcutKey::Char(c),
        };
        h.send(Input::key(key));
    }
    h.send(Input::key(ShortcutKey::Enter));
}

/// A window whose shell has listed `a.txt` and `b.txt`, two files in its folder.
fn listed(name: &str, probe: &Probe) -> Harness {
    let mut h = harness(name, probe);
    let home = scratch_path(name).join("home");
    for file in ["a.txt", "b.txt"] {
        std::fs::write(home.join(file), format!("{file} says hello\n")).expect("scratch file");
    }
    wait_until(&mut h, probe, "the prompt", |s| s.contains('$'));
    h.send(Input::click(Point {
        x: Px(300.0),
        y: Px(200.0),
    }));
    h.advance(Duration::from_millis(200));
    type_line(&mut h, "echo a.txt b.txt");
    wait_until(&mut h, probe, "the listing", |s| {
        s.lines().any(|l| l.trim_end() == "a.txt b.txt")
    });
    h
}

/// ⌘-clicks down the third character of each row until a viewer opens. The pane starts 60px
/// left of the focusing click at x=300 (the tab sidebar is to its left), and its gutter selects
/// a block, so the click goes a few cells in. The listing's row is not
/// known to the test (the font decides the rows' height), so every row of the pane is tried; a
/// click on anything that is not a file does nothing.
fn open_by_clicking(h: &mut Harness) {
    let mut y = 14.0;
    while viewers(h) == 0 && y < 320.0 {
        command_click(
            h,
            Point {
                x: Px(272.0),
                y: Px(y),
            },
        );
        y += 4.0;
    }
    let start = Instant::now();
    while viewers(h) == 0 {
        h.advance(Duration::from_millis(25));
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "no viewer opened"
        );
    }
    h.advance(Duration::from_millis(300));
}

#[test]
fn command_click_on_a_listed_file_opens_a_viewer_beside_the_terminal() {
    if !tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = listed("viewer-open", &probe);
    assert_eq!((terminals(&h), viewers(&h)), (1, 0));
    open_by_clicking(&mut h);
    assert_eq!((terminals(&h), viewers(&h)), (1, 1), "{}", h.html());
}

#[test]
fn command_w_closes_the_viewer_and_leaves_the_terminal() {
    if !tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = listed("viewer-close", &probe);
    open_by_clicking(&mut h);
    chord(&mut h, 'w');
    assert_eq!((terminals(&h), viewers(&h)), (1, 0), "{}", h.html());
}

#[test]
fn escape_gives_the_keyboard_back_to_the_terminal() {
    if !tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = listed("viewer-escape", &probe);
    open_by_clicking(&mut h);
    // The viewer has the keyboard: what is typed does not reach the shell.
    type_line(&mut h, "echo lost");
    h.advance(Duration::from_millis(300));
    assert!(!probe.screen().contains("echo lost"), "{}", probe.screen());
    h.send(Input::key(ShortcutKey::Escape));
    h.advance(Duration::from_millis(300));
    // Now it is the shell's, and the viewer is still there.
    type_line(&mut h, "echo found");
    wait_until(&mut h, &probe, "the shell to hear it", |s| {
        s.lines().any(|l| l.trim_end() == "found")
    });
    assert_eq!(viewers(&h), 1);
}
