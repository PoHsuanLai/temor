//! A block's card in the list (PLAN: block list): its head says how a failed command ended, its
//! tools show while the pointer is over it, a click picks its block and the copy key takes it
//! whole, and its menu acts on it. In ds-harness: events dispatched to the window, the DOM and the
//! clipboard read back, never pixels.

use std::time::Duration;

use ds::base::press::PointerButton;
use ds::prelude::{Point, Px};
use ds_harness::{Harness, Input};
use temor_ui::view::probe::Probe;

use super::drive::{Rect, chord, rects, started, type_line, wait_html, wait_until};
use super::support::window_tools_present;

/// A card head's opening tag: the stylesheet names the class too.
const HEAD: &str = "temor-bseg temor-blk-head\"";

/// Runs `line` and waits until its last output line `last` is on the screen and its card is in
/// the list with how it ended.
fn run_to_end(h: &mut Harness, probe: &Probe, line: &str, last: &str) {
    let before = h.html().matches(HEAD).count();
    type_line(h, line);
    wait_until(h, probe, "the command's last line", |s| {
        s.lines().any(|l| l.trim_end() == last)
    });
    wait_html(h, "the command's card", |html| {
        html.matches(HEAD).count() > before
    });
    h.advance(Duration::from_millis(300));
}

/// The rows of each card, top to bottom.
fn rows(h: &Harness) -> Vec<Rect> {
    let mut found = rects(h, ".temor-blk-rows");
    found.sort_by(|a, b| a.y.total_cmp(&b.y));
    found
}

/// The middle of a box, a little in from its left.
fn on(r: Rect) -> Point {
    Point {
        x: Px((r.x + 120.0) as f32),
        y: Px((r.y + r.h / 2.0) as f32),
    }
}

/// The centres of the open menu's items, top to bottom.
fn items(h: &Harness) -> Vec<Point> {
    let mut found = rects(h, ".ds-menu-item");
    found.sort_by(|a, b| a.y.total_cmp(&b.y));
    found
        .into_iter()
        .map(|r| Point {
            x: Px((r.x + r.w / 2.0) as f32),
            y: Px((r.y + r.h / 2.0) as f32),
        })
        .collect()
}

/// One right-click: the move, the press and the release, which Blitz delivers as `contextmenu`.
fn right_click(h: &mut Harness, at: Point) {
    h.send(Input::press(at, PointerButton::Secondary));
    h.advance(Duration::from_millis(300));
}

#[test]
fn a_failed_card_says_its_exit_status_in_its_head_however_long_its_command() {
    if !window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("cards-status", &probe);
    run_to_end(&mut h, &probe, "echo short; (exit 3)", "short");
    let long = format!(": {}; echo long; (exit 4)", "x".repeat(140));
    run_to_end(&mut h, &probe, &long, "long");
    let html = h.html();
    assert!(
        html.contains(">exit 3 \u{b7} "),
        "the short card's status:\n{html}"
    );
    assert!(
        html.contains(">exit 4 \u{b7} "),
        "the long card's status:\n{html}"
    );
    assert!(
        !html.contains("Failed with status"),
        "the status sentence is back"
    );
}

#[test]
fn hovering_a_card_shows_its_tools_and_the_first_copies_the_output() {
    if !window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("cards-tools", &probe);
    run_to_end(&mut h, &probe, "echo hover-one", "hover-one");
    run_to_end(&mut h, &probe, "echo hover-two", "hover-two");
    h.send(Input::pointer_move(Point {
        x: Px(1.0),
        y: Px(1.0),
    }));
    h.advance(Duration::from_millis(200));
    let none = h.html().matches("class=\"temor-blk-tools\"").count();
    let first = rows(&h)[0];
    h.send(Input::pointer_move(on(first)));
    h.advance(Duration::from_millis(200));
    assert!(
        h.html().matches("class=\"temor-blk-tools\"").count() > none,
        "no tools over the card:\n{}",
        h.html()
    );
    let tools = rects(&h, ".temor-blk-tools .ds-button");
    let copy = tools
        .iter()
        .filter(|r| r.y < first.y)
        .min_by(|a, b| a.x.total_cmp(&b.x))
        .copied()
        .unwrap_or_else(|| panic!("no tool on the hovered card:\n{}", h.html()));
    h.send(Input::click(copy.centre()));
    h.advance(Duration::from_millis(200));
    let copied = h.clipboard_text().unwrap_or_default();
    assert!(
        copied.contains("hover-one") && !copied.contains("echo"),
        "copied {copied:?}"
    );
}

#[test]
fn the_alternate_screen_shows_no_cards() {
    if !window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("cards-alternate", &probe);
    run_to_end(&mut h, &probe, "echo before", "before");
    type_line(&mut h, "printf '\\033[?1049h'; sleep 30");
    wait_html(&mut h, "the cards to go", |html| !html.contains(HEAD));
}

#[test]
fn a_click_picks_a_block_and_the_copy_key_takes_it_whole_and_select_all_takes_them_all() {
    if !window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("cards-pick", &probe);
    run_to_end(&mut h, &probe, "echo pick-one", "pick-one");
    run_to_end(&mut h, &probe, "echo pick-two", "pick-two");
    let first = rects(&h, ".temor-blk-head")
        .into_iter()
        .min_by(|a, b| a.y.total_cmp(&b.y))
        .expect("a head");
    h.send(Input::click(on(first)));
    h.advance(Duration::from_millis(200));
    wait_html(&mut h, "the block picked", |html| {
        html.contains("data-picked=\"true\"")
    });
    chord(&mut h, 'c');
    assert_eq!(
        h.clipboard_text().unwrap_or_default(),
        "echo pick-one\npick-one",
        "the clicked block, command and output"
    );
    // The platform's select all picks every block.
    chord(&mut h, 'a');
    chord(&mut h, 'c');
    assert_eq!(
        h.clipboard_text().unwrap_or_default(),
        "echo pick-one\npick-one\n\necho pick-two\npick-two"
    );
}

#[test]
fn a_right_click_on_a_card_opens_its_menu_and_the_menu_acts_on_the_block() {
    if !window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("cards-menu", &probe);
    run_to_end(&mut h, &probe, "echo menu-one", "menu-one");
    run_to_end(&mut h, &probe, "echo menu-two", "menu-two");
    assert!(items(&h).is_empty(), "a menu before the right-click");
    let second = on(rows(&h)[1]);
    right_click(&mut h, second);
    let html = h.html();
    for title in [
        "Copy Output",
        "Put in Prompt",
        "Bookmark",
        "Fold Output",
        "Select All Commands",
    ] {
        assert!(html.contains(title), "no {title:?} in the menu:\n{html}");
    }
    // Copy, Copy Command, Copy Output, Put in Prompt, Bookmark, Fold Output, Select All Commands.
    let found = items(&h);
    assert_eq!(found.len(), 7, "the menu's items:\n{html}");
    h.send(Input::click(found[2]));
    h.advance(Duration::from_millis(300));
    assert_eq!(
        h.clipboard_text().unwrap_or_default(),
        "menu-two",
        "Copy Output: the right-clicked block's output"
    );
    assert!(items(&h).is_empty(), "the menu closed after the pick");
    // Bookmarked, the card shows its star and the item turns around.
    assert!(
        !h.html().contains("class=\"temor-blk-mark\""),
        "a star before"
    );
    right_click(&mut h, second);
    h.send(Input::click(items(&h)[4]));
    h.advance(Duration::from_millis(300));
    wait_html(&mut h, "the star", |html| {
        html.matches("class=\"temor-blk-mark\"").count() == 1
    });
    right_click(&mut h, second);
    assert!(
        h.html().contains("Remove Bookmark"),
        "the bookmarked block's menu:\n{}",
        h.html()
    );
}
