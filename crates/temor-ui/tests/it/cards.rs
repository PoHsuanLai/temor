//! The sticky command header and the block toolbar in the window (PLAN §9 blocks), in
//! ds-harness: events dispatched to the window, the DOM and the probe's text read back, never
//! pixels.

use std::time::Duration;

use ds::base::vocab::ShortcutKey;
use ds::prelude::{Point, Px};
use ds_harness::{Driver, Harness, Input};
use temor_ui::view::probe::Probe;

use super::support::tools_present;
use super::window::{harness, wait_until};

const HEADER: &str = "temor-block-header";
const TOOLS: &str = "temor-block-tools";

/// Types `line` and presses Return.
fn run_line(h: &mut Harness, line: &str) {
    for c in line.chars() {
        let key = match c {
            ' ' => ShortcutKey::Space,
            c => ShortcutKey::Char(c),
        };
        h.send(Input::key(key));
    }
    h.send(Input::key(ShortcutKey::Enter));
}

/// A window whose pane has the keyboard and shows its first prompt.
fn ready(name: &str, probe: &Probe) -> Harness {
    let mut h = harness(name, probe);
    wait_until(&mut h, probe, "the prompt", |s| s.contains('$'));
    h.send(Input::click(Point {
        x: Px(500.0),
        y: Px(200.0),
    }));
    h.advance(Duration::from_millis(100));
    h
}

/// Runs `line` and waits until its last output line `last` is on the screen and the block has
/// ended (the next prompt is drawn under it).
fn run_to_end(h: &mut Harness, probe: &Probe, line: &str, last: &str) {
    run_line(h, line);
    wait_until(h, probe, "the command's last line", |s| {
        s.lines().any(|l| l.trim_end() == last)
    });
    h.advance(Duration::from_millis(300));
}

/// Moves the pointer down the pane until the toolbar shows; whether it did.
fn hover_until_tools(h: &mut Harness) -> bool {
    for y in (0..480_u16).step_by(6) {
        h.send(Input::pointer_move(Point {
            x: Px(500.0),
            y: Px(f32::from(y)),
        }));
        h.advance(Duration::from_millis(30));
        if h.html().contains(TOOLS) {
            return true;
        }
    }
    false
}

#[test]
fn the_header_shows_the_command_of_a_scrolled_block_and_a_press_brings_the_prompt_back() {
    if !tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = ready("cards-header", &probe);
    // Step 1: nothing has scrolled, so no header.
    assert!(!h.html().contains(HEADER), "a header with nothing scrolled");
    // Step 2: the output pushes the prompt out of the top of the pane.
    run_to_end(&mut h, &probe, "seq 1 100", "100");
    let html = h.html();
    assert!(html.contains(HEADER), "no header:\n{html}");
    assert!(html.contains("seq 1 100"), "the header lacks the command");
    // Step 3: pressing it scrolls the prompt back to the top; the header goes.
    let at = h
        .centre(&format!(".{HEADER} .ds-button"))
        .unwrap_or_else(|| panic!("no header button:\n{}", h.html()));
    h.send(Input::click(at));
    h.advance(Duration::from_millis(200));
    wait_until(&mut h, &probe, "the prompt at the top", |s| {
        s.lines().next().is_some_and(|l| l.contains("seq 1 100"))
    });
    assert!(
        !h.html().contains(HEADER),
        "the header stays over its own prompt"
    );
}

#[test]
fn hovering_a_block_shows_its_toolbar_and_copy_command_copies_the_command() {
    if !tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = ready("cards-tools", &probe);
    assert!(!h.html().contains(TOOLS), "a toolbar with nothing hovered");
    run_to_end(&mut h, &probe, "echo hover-me", "hover-me");
    assert!(hover_until_tools(&mut h), "no toolbar:\n{}", h.html());
    // The first button is Copy command.
    let at = h
        .centre(&format!(".{TOOLS} .ds-button"))
        .unwrap_or_else(|| panic!("no toolbar button:\n{}", h.html()));
    h.send(Input::click(at));
    h.advance(Duration::from_millis(200));
    assert_eq!(h.clipboard_text().as_deref(), Some("echo hover-me"));
}

#[test]
fn neither_the_header_nor_the_toolbar_shows_on_the_alternate_screen() {
    if !tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = ready("cards-alternate", &probe);
    run_to_end(&mut h, &probe, "seq 1 100", "100");
    // Step 1: both show on the shell's screen.
    assert!(h.html().contains(HEADER), "no header before");
    assert!(hover_until_tools(&mut h), "no toolbar before");
    // Step 2: a full-screen program takes the screen; neither shows, however the pointer moves.
    run_line(&mut h, "printf '\\033[?1049h'");
    h.advance(Duration::from_millis(500));
    assert!(
        !h.html().contains(HEADER),
        "the header stays on the alternate screen"
    );
    assert!(
        !hover_until_tools(&mut h),
        "the toolbar stays on the alternate screen"
    );
}
