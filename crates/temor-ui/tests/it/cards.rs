//! The sticky command header, the block toolbar, picking blocks and the block menu in the
//! window (PLAN §9 blocks), in ds-harness: events dispatched to the window, the DOM and the
//! probe's text read back, never pixels.

use std::time::Duration;

use ds::base::press::PointerButton;
use ds::base::vocab::ShortcutKey;
use ds::prelude::{Point, Px};
use ds_harness::{Harness, Input};
use temor_ui::view::probe::Probe;

use super::drive::{chord, ready, type_line, wait_until};
use super::support::tools_present;
use super::window::harness;

const HEADER: &str = "temor-block-header";
const TOOLS: &str = "temor-block-tools";

/// Whether an element with `class` is in the page (the stylesheet names it too).
fn shows(html: &str, class: &str) -> bool {
    html.contains(&format!("class=\"{class}"))
}

/// Runs `line` and waits until its last output line `last` is on the screen and the block has
/// ended (the next prompt is drawn under it).
fn run_to_end(h: &mut Harness, probe: &Probe, line: &str, last: &str) {
    type_line(h, line);
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
        if shows(&h.html(), TOOLS) {
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
    assert!(!shows(&h.html(), HEADER), "a header with nothing scrolled");
    // Step 2: the output pushes the prompt out of the top of the pane.
    run_to_end(&mut h, &probe, "seq 1 100", "100");
    let html = h.html();
    assert!(shows(&html, HEADER), "no header:\n{html}");
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
        !shows(&h.html(), HEADER),
        "the header stays over its own prompt"
    );
}

#[test]
fn hovering_a_block_shows_its_tools_and_the_first_copies_the_output() {
    if !tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = ready("cards-tools", &probe);
    assert!(!shows(&h.html(), TOOLS), "a toolbar with nothing hovered");
    run_to_end(&mut h, &probe, "echo hover-me", "hover-me");
    assert!(hover_until_tools(&mut h), "no toolbar:\n{}", h.html());
    // The first button is Copy output.
    let at = h
        .centre(&format!(".{TOOLS} .ds-button"))
        .unwrap_or_else(|| panic!("no toolbar button:\n{}", h.html()));
    h.send(Input::click(at));
    h.advance(Duration::from_millis(200));
    let copied = h.clipboard_text().unwrap_or_default();
    assert!(
        copied.contains("hover-me") && !copied.contains("echo"),
        "copied {copied:?}"
    );
}

#[test]
fn the_toolbar_goes_when_the_pointer_leaves_the_pane_and_the_wheel_keeps_it_on_what_is_under_it() {
    if !tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = ready("cards-leave", &probe);
    run_to_end(&mut h, &probe, "echo leave-me", "leave-me");
    assert!(hover_until_tools(&mut h), "no toolbar:\n{}", h.html());
    // The wheel turns with the pointer still: the hover is recomputed, not dropped.
    h.send(Input::Wheel {
        at: Point {
            x: Px(500.0),
            y: Px(200.0),
        },
        dx: Px(0.0),
        dy: Px(0.0),
    });
    h.advance(Duration::from_millis(50));
    // Out over the sidebar: nothing is hovered any more.
    h.send(Input::pointer_move(Point {
        x: Px(20.0),
        y: Px(200.0),
    }));
    h.advance(Duration::from_millis(100));
    assert!(
        !shows(&h.html(), TOOLS),
        "the toolbar stays after the pointer left:\n{}",
        h.html()
    );
}

#[test]
fn a_failed_card_says_its_exit_status_in_its_header_and_no_sentence() {
    if !tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = ready("cards-meta", &probe);
    run_to_end(&mut h, &probe, "echo oops; (exit 3)", "oops");
    let html = h.html();
    assert!(html.contains("exit 3 \u{b7} "), "no exit status:\n{html}");
    assert!(
        !html.contains("Failed with status"),
        "the status sentence is back"
    );
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
    assert!(shows(&h.html(), HEADER), "no header before");
    assert!(hover_until_tools(&mut h), "no toolbar before");
    // Step 2: a full-screen program takes the screen; neither shows, however the pointer moves.
    type_line(&mut h, "printf '\\033[?1049h'");
    h.advance(Duration::from_millis(500));
    assert!(
        !shows(&h.html(), HEADER),
        "the header stays on the alternate screen"
    );
    assert!(
        !hover_until_tools(&mut h),
        "the toolbar stays on the alternate screen"
    );
}

/// The tops and bottoms of the elements matching `selector`, in window pixels.
fn spans(h: &Harness, selector: &str) -> Vec<(f64, f64)> {
    h.with_doc(|doc| {
        doc.query_selector_all(selector)
            .map(|found| found.into_iter().collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|id| doc.get_client_bounding_rect(id))
            .map(|r| (r.y, r.y + r.height))
            .collect()
    })
}

#[test]
fn a_card_meta_steps_aside_where_the_header_text_reaches_it() {
    if !tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = ready("cards-meta-room", &probe);
    run_to_end(&mut h, &probe, "echo short; (exit 3)", "short");
    // A command longer than the row: its header line runs to the right edge.
    let long = format!(": {}; echo long; (exit 4)", "x".repeat(140));
    run_to_end(&mut h, &probe, &long, "long");
    let html = h.html();
    assert!(
        html.contains("exit 3 \u{b7} "),
        "the short card's meta:\n{html}"
    );
    assert!(
        !html.contains("exit 4 \u{b7} "),
        "the long card's meta lies over its command"
    );
}

#[test]
fn the_find_bar_hides_the_card_meta_it_would_cover() {
    if !tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = ready("cards-meta-find", &probe);
    run_to_end(&mut h, &probe, "echo one; (exit 3)", "one");
    run_to_end(&mut h, &probe, "echo two; (exit 4)", "two");
    let top_meta = |h: &Harness| spans(h, ".temor-card-meta").first().map(|s| s.0);
    let first = top_meta(&h).expect("the first card's meta");
    h.send(Input::chord(
        &[ShortcutKey::Ctrl, ShortcutKey::Shift],
        ShortcutKey::Char('f'),
    ));
    h.advance(Duration::from_millis(400));
    let bar = spans(&h, ".temor-find");
    assert_eq!(bar.len(), 1, "the find bar opened");
    let bottom = bar[0].1;
    assert!(
        first < bottom,
        "the first card's meta starts under the bar: {first} vs {bottom}"
    );
    for (top, _) in spans(&h, ".temor-card-meta") {
        assert!(
            top >= bottom,
            "a meta at {top} lies under the find bar (to {bottom})"
        );
    }
    assert!(
        h.html().contains("exit 4 \u{b7} "),
        "the meta below the bar stays"
    );
}

/// The middle of the header row of the `n`th card on screen, a little into the grid: a point on
/// that block. The row holds the card's meta, or the toolbar of the hovered card in its place.
fn on_card(h: &Harness, n: usize) -> Point {
    let mut rows = spans(h, ".temor-card-meta");
    rows.extend(spans(h, ".temor-block-tools"));
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (top, bottom) = rows
        .get(n)
        .copied()
        .unwrap_or_else(|| panic!("no card {n}:\n{}", h.html()));
    Point {
        x: Px(300.0),
        y: Px(((top + bottom) / 2.0) as f32),
    }
}

/// One right-click: the move, the press and the release, which Blitz delivers as `contextmenu`.
fn right_click(h: &mut Harness, at: Point) {
    h.send(Input::press(at, PointerButton::Secondary));
    h.advance(Duration::from_millis(300));
}

#[test]
fn a_click_picks_a_block_and_the_copy_key_takes_it_whole_and_select_all_takes_them_all() {
    if !tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = ready("cards-pick", &probe);
    run_to_end(&mut h, &probe, "echo pick-one", "pick-one");
    run_to_end(&mut h, &probe, "echo pick-two", "pick-two");
    h.send(Input::click(on_card(&h, 0)));
    h.advance(Duration::from_millis(200));
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
fn a_right_click_on_a_block_opens_its_menu_and_the_menu_acts_on_the_block() {
    if !tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = ready("cards-menu", &probe);
    run_to_end(&mut h, &probe, "echo menu-one", "menu-one");
    run_to_end(&mut h, &probe, "echo menu-two", "menu-two");
    let items = |h: &Harness| {
        h.with_doc(|doc| {
            doc.query_selector_all(".ds-menu-item")
                .map(|found| found.into_iter().collect::<Vec<_>>())
                .unwrap_or_default()
                .into_iter()
                .filter_map(|id| doc.get_client_bounding_rect(id))
                .map(|r| Point {
                    x: Px((r.x + r.width / 2.0) as f32),
                    y: Px((r.y + r.height / 2.0) as f32),
                })
                .collect::<Vec<_>>()
        })
    };
    assert!(items(&h).is_empty(), "a menu before the right-click");
    let second = on_card(&h, 1);
    right_click(&mut h, second);
    let html = h.html();
    for title in [
        "Copy Output",
        "Put in Prompt",
        "Bookmark",
        "Select All Commands",
    ] {
        assert!(html.contains(title), "no {title:?} in the menu:\n{html}");
    }
    // Copy, Copy Command, Copy Output, Put in Prompt, Bookmark, Select All Commands.
    let found = items(&h);
    assert_eq!(found.len(), 6, "the menu's items:\n{html}");
    h.send(Input::click(found[2]));
    h.advance(Duration::from_millis(300));
    assert_eq!(
        h.clipboard_text().unwrap_or_default(),
        "menu-two",
        "Copy Output: the right-clicked block's output"
    );
    assert!(items(&h).is_empty(), "the menu closed after the pick");
    // Bookmarked, the item turns around.
    right_click(&mut h, second);
    h.send(Input::click(items(&h)[4]));
    h.advance(Duration::from_millis(300));
    right_click(&mut h, second);
    assert!(
        h.html().contains("Remove Bookmark"),
        "the bookmarked block's menu:\n{}",
        h.html()
    );
}
