//! A tab's close button, the tab selection's wash, and the room around the terminal.

use std::time::Duration;

use ds_harness::Input;
use temor_ui::view::probe::Probe;

use super::drive::{chord, rect, rects, started, type_line, wait_until};

const ROWS: &str = ".temor-side .temor-tab-box";

#[test]
fn a_tabs_close_button_closes_that_tab_and_the_others_stay() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("tab-close", &probe);
    chord(&mut h, 'n');
    chord(&mut h, 'n');
    assert_eq!(rects(&h, ROWS).len(), 3, "three tabs");
    let second = rects(&h, ".temor-tab-close")[1].centre();
    h.send(Input::click(second));
    h.advance(Duration::from_millis(400));
    assert_eq!(rects(&h, ROWS).len(), 2, "the second tab is gone");
    wait_until(&mut h, &probe, "a prompt", |s| s.contains('$'));
    type_line(&mut h, "echo alive$((40+2))");
    wait_until(&mut h, &probe, "alive42", |s| s.contains("alive42"));
}

#[test]
fn the_close_button_sits_inside_its_row_at_the_end() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let h = started("tab-close-place", &probe);
    let row = rects(&h, ROWS)[0];
    let close = rects(&h, ".temor-tab-close")[0];
    assert!(
        close.x >= row.x && close.x + close.w <= row.x + row.w,
        "inside: {close:?} in {row:?}"
    );
    assert!(
        row.x + row.w - (close.x + close.w) < 24.0,
        "at the row's end: {close:?} in {row:?}"
    );
}

#[test]
fn the_terminal_has_room_around_it_in_the_sidebars_measure() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let h = started("pane-room", &probe);
    let pane = rect(&h, ".temor-pane");
    let keys = rect(&h, ".temor-keys");
    // The console's bar heads the pane, the terminal under it.
    let bar = rect(&h, ".temor-console-bar");
    assert!(
        (keys.x - pane.x - 10.0).abs() < 1.0,
        "left room: {pane:?} {keys:?}"
    );
    assert!(
        (bar.y - pane.y - 8.0).abs() < 1.0,
        "top room: {pane:?} {bar:?}"
    );
    assert!(
        (keys.y - (bar.y + bar.h) - 4.0).abs() < 1.0,
        "room under the bar: {bar:?} {keys:?}"
    );
    assert!(
        (pane.x + pane.w - (keys.x + keys.w) - 10.0).abs() < 1.0,
        "right room: {pane:?} {keys:?}"
    );
}
