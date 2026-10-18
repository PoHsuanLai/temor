//! The window's chrome pushed by size: the sidebar hidden and shown again keeps the width it was
//! dragged to, and a menu open while the window is resized stays inside it and takes clicks.

use std::time::Duration;

use ds::base::press::PointerButton;
use ds::base::vocab::ShortcutKey;
use ds::prelude::{Point, Px};
use ds_blitz::Extent;
use ds_harness::Input;
use temor_ui::view::probe::Probe;

use super::resizing::{rect, rects};
use super::window::{harness, wait_until};

/// The window's own split: the sidebar is its first pane.
const WINDOW: &str = ".ds-split[aria-label=\"Window\"]";

#[test]
fn a_sidebar_hidden_and_shown_again_keeps_the_width_it_was_dragged_to() {
    if !super::support::tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness("resize-sidebar-width", &probe);
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    let side = format!("{WINDOW} > .ds-split-pane");
    let first = rect(&h, &side);
    let grip = rect(&h, &format!("{WINDOW} > .ds-split-divider")).centre();
    h.send(Input::drag(
        grip,
        Point {
            x: Px(grip.x.0 + 60.0),
            y: grip.y,
        },
        12,
    ));
    h.advance(Duration::from_millis(400));
    let dragged = rect(&h, &side);
    assert!(
        (dragged.w - first.w - 60.0).abs() < 4.0,
        "the sidebar follows the pointer: {first:?} -> {dragged:?}"
    );
    for round in 1..=2 {
        for _ in 0..2 {
            h.send(Input::chord(
                &[ShortcutKey::Super, ShortcutKey::Shift],
                ShortcutKey::Char('l'),
            ));
            h.advance(Duration::from_millis(600));
        }
        let back = rect(&h, &side);
        assert!(
            (back.w - dragged.w).abs() < 4.0,
            "round {round}: hidden and shown again, the width is kept: {dragged:?} -> {back:?}"
        );
    }
}

/// The tab's menu is open while the window is made smaller and larger: whatever of it is still
/// drawn lies inside the window, and a click on its first item lands on it (Pin Tab).
#[test]
fn a_tab_menu_open_while_the_window_is_resized_stays_inside_it_and_takes_clicks() {
    if !super::support::tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness("resize-menu-open", &probe);
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    let row = rects(&h, ".temor-side .temor-tab-box")[0].centre();
    h.send(Input::press(row, PointerButton::Secondary));
    h.advance(Duration::from_millis(300));
    assert!(
        !rects(&h, ".ds-menu-item").is_empty(),
        "the tab's menu opens:\n{}",
        h.html()
    );
    for (width, height) in [(900, 500), (760, 300), (600, 400), (1280, 800), (400, 360)] {
        h.resize_window(Extent::new(width, height));
        h.advance(Duration::from_millis(400));
        for item in rects(&h, ".ds-menu-item") {
            assert!(
                item.x >= -0.5
                    && item.right() <= f64::from(width) + 0.5
                    && item.y >= -0.5
                    && item.y + item.h <= f64::from(height) + 0.5,
                "{width}x{height}: the menu leaves the window: {item:?}"
            );
        }
    }
    h.resize_window(Extent::new(1280, 800));
    h.advance(Duration::from_millis(400));
    // Open still, the first item is where it is drawn; closed, a new menu opens and its first
    // item is. Either way the click lands: the tab is pinned.
    if rects(&h, ".ds-menu-item").is_empty() {
        let row = rects(&h, ".temor-side .temor-tab-box")[0].centre();
        h.send(Input::press(row, PointerButton::Secondary));
        h.advance(Duration::from_millis(300));
    }
    let items = rects(&h, ".ds-menu-item");
    assert!(!items.is_empty(), "a menu to click:\n{}", h.html());
    h.send(Input::click(items[0].centre()));
    h.advance(Duration::from_millis(400));
    assert_eq!(
        rects(&h, ".temor-side .temor-pin").len(),
        1,
        "the click landed on Pin Tab:\n{}",
        h.html()
    );
}
