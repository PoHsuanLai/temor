//! Driving a window from a test: typing, chords, clicks, waiting, and measuring boxes. Every
//! file that drives a window uses these, so the settle times and time limits are the same
//! everywhere.

use std::time::{Duration, Instant};

use ds::base::vocab::ShortcutKey;
use ds::prelude::{Point, Px};
use ds_harness::{Harness, Input};
use temor_ui_base::probe::Probe;

use super::window::harness;

/// How long the window gets to show what a test waits for.
pub(super) const LIMIT: Duration = Duration::from_secs(30);
/// How long the window gets to take in a chord or a click before the next step.
const SETTLE: Duration = Duration::from_millis(400);
const CLICK_SETTLE: Duration = Duration::from_millis(200);
const STEP: Duration = Duration::from_millis(25);

/// Types `text` without pressing Return.
pub(super) fn type_text(h: &mut Harness, text: &str) {
    for c in text.chars() {
        let key = match c {
            ' ' => ShortcutKey::Space,
            c => ShortcutKey::Char(c),
        };
        h.send(Input::key(key));
    }
}

/// Types `text` and presses Return.
pub(super) fn type_line(h: &mut Harness, text: &str) {
    type_text(h, text);
    h.send(Input::key(ShortcutKey::Enter));
}

/// Presses `held` and `key` together, and lets the window take it in.
pub(super) fn chord_with(h: &mut Harness, held: &[ShortcutKey], key: char) {
    h.send(Input::chord(held, ShortcutKey::Char(key)));
    h.advance(SETTLE);
}

/// The window's app chord: Ctrl and Shift with `key`.
pub(super) fn chord(h: &mut Harness, key: char) {
    chord_with(h, &[ShortcutKey::Ctrl, ShortcutKey::Shift], key);
}

/// Clicks the middle of the element `selector` names, once there is one; panics, with the page,
/// if none comes.
pub(super) fn press(h: &mut Harness, selector: &str) {
    // What is pressed may be drawn a little after what the test waited for.
    let start = Instant::now();
    let at = loop {
        if let Some(at) = h.centre(selector) {
            break at;
        }
        h.advance(STEP);
        assert!(
            start.elapsed() < LIMIT,
            "nothing matches {selector}:\n{}",
            h.html()
        );
    };
    h.send(Input::click(at));
    h.advance(CLICK_SETTLE);
}

/// Clicks in the first pane, which gives it the keyboard as a person's click would.
pub(super) fn focus_pane(h: &mut Harness) {
    h.send(Input::click(Point {
        x: Px(300.0),
        y: Px(200.0),
    }));
    h.advance(CLICK_SETTLE);
}

/// Steps the window until `ok` holds for the page's markup, or panics with the page.
pub(super) fn wait_html(h: &mut Harness, what: &str, ok: impl Fn(&str) -> bool) {
    let start = Instant::now();
    while !ok(&h.html()) {
        h.advance(STEP);
        assert!(
            start.elapsed() < LIMIT,
            "timed out waiting for {what}:\n{}",
            h.html()
        );
    }
}

/// Whether the keyboard is in a terminal: its region, or its composer's field.
fn keyboard_in_pane(h: &Harness) -> bool {
    h.with_doc(|doc| {
        let mut at = doc.get_focussed_node_id();
        while let Some(id) = at {
            let node = doc.get_node(id)?;
            let class = node
                .element_data()
                .and_then(|el| el.attrs().iter().find(|a| &*a.name.local == "class"))
                .map(|a| a.value.clone())
                .unwrap_or_default();
            if class
                .split_whitespace()
                .any(|c| c == "temor-keys" || c == "temor-blk-composer")
            {
                return Some(());
            }
            at = node.parent;
        }
        None
    })
    .is_some()
}

/// The element with the keyboard: its tag and classes.
fn focused(h: &Harness) -> Option<String> {
    h.with_doc(|doc| {
        let el = doc.get_node(doc.get_focussed_node_id()?)?.element_data()?;
        let class = el.attrs().iter().find(|a| &*a.name.local == "class");
        Some(format!(
            "{} {:?}",
            &*el.name.local,
            class.map(|a| a.value.clone())
        ))
    })
}

/// Steps the window until a terminal has the keyboard (a click elsewhere took it, and the
/// window gives it back a little later), or panics with the page.
pub(super) fn wait_keyboard(h: &mut Harness) {
    let start = Instant::now();
    while !keyboard_in_pane(h) {
        h.advance(STEP);
        assert!(
            start.elapsed() < LIMIT,
            "timed out waiting for the keyboard in a terminal; it is in {:?}:\n{}",
            focused(h),
            h.html()
        );
    }
}

/// Steps the window until `ok` holds for the terminal's text, or panics with it.
pub(super) fn wait_until(h: &mut Harness, probe: &Probe, what: &str, ok: impl Fn(&str) -> bool) {
    wait_screen(h, probe, LIMIT, what, ok);
}

/// Like [`wait_until`], with a time limit of the test's choosing.
pub(super) fn wait_screen(
    h: &mut Harness,
    probe: &Probe,
    limit: Duration,
    what: &str,
    ok: impl Fn(&str) -> bool,
) {
    let start = Instant::now();
    while !ok(&probe.screen()) {
        h.advance(STEP);
        assert!(
            start.elapsed() < limit,
            "timed out waiting for {what}:\n{}\n{}",
            probe.screen(),
            h.html()
        );
    }
}

/// A window that shows its first prompt.
pub(super) fn started(name: &str, probe: &Probe) -> Harness {
    let mut h = harness(name, probe);
    wait_until(&mut h, probe, "the prompt", |s| s.contains('$'));
    h
}

/// How many tab rows the sidebar shows.
pub(super) fn rows(h: &Harness) -> usize {
    h.html().matches("class=\"temor-tab-box\"").count()
}

/// How many terminal panes the tab in front shows.
pub(super) fn panes(h: &Harness) -> usize {
    h.html().matches("aria-label=\"Terminal\"").count()
}

/// A box: left, top, width, height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Rect {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) w: f64,
    pub(super) h: f64,
}

impl Rect {
    pub(super) fn right(self) -> f64 {
        self.x + self.w
    }

    pub(super) fn centre(self) -> Point {
        Point {
            x: Px((self.x + self.w / 2.0) as f32),
            y: Px((self.y + self.h / 2.0) as f32),
        }
    }
}

/// The boxes of the elements `selector` finds, in document order.
pub(super) fn rects(h: &Harness, selector: &str) -> Vec<Rect> {
    h.with_doc(|doc| {
        doc.query_selector_all(selector)
            .map(|found| found.into_iter().collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|id| doc.get_client_bounding_rect(id))
            .map(|r| Rect {
                x: r.x,
                y: r.y,
                w: r.width,
                h: r.height,
            })
            .collect()
    })
}

/// The box of the one element `selector` finds.
pub(super) fn rect(h: &Harness, selector: &str) -> Rect {
    let found = rects(h, selector);
    assert_eq!(found.len(), 1, "one {selector}: {found:?}");
    found[0]
}

/// The box of the open menu's item that says `title`, once the open menus have come to rest: a
/// press while a menu is still moving in can land beside the item, which closes the menu and
/// picks nothing. Panics, with the page, if no item says `title`.
pub(super) fn menu_item(h: &mut Harness, title: &str) -> Rect {
    still_rects(h, ".ds-menu-item");
    menu_item_now(h, title)
}

/// The box of the open menu's item that says `title` as it is now.
fn menu_item_now(h: &Harness, title: &str) -> Rect {
    let found = h.with_doc(|doc| {
        doc.query_selector_all(".ds-menu-item")
            .map(|found| found.into_iter().collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .find(|id| {
                doc.get_node(*id)
                    .is_some_and(|node| node.text_content().trim() == title)
            })
            .and_then(|id| doc.get_client_bounding_rect(id))
            .map(|r| Rect {
                x: r.x,
                y: r.y,
                w: r.width,
                h: r.height,
            })
    });
    found.unwrap_or_else(|| panic!("no menu item {title:?}:\n{}", h.html()))
}

/// The boxes of `selector` once they have held still for a few steps (a list still laying out
/// or easing its scroll moves them under a click), or panics.
pub(super) fn still_rects(h: &mut Harness, selector: &str) -> Vec<Rect> {
    let start = Instant::now();
    let mut last = rects(h, selector);
    let mut held = 0;
    while held < 4 {
        h.advance(STEP);
        let now = rects(h, selector);
        held = if now == last { held + 1 } else { 0 };
        last = now;
        assert!(start.elapsed() < LIMIT, "{selector} never held still");
    }
    last
}

/// Steps the window until the field `selector` names has the keyboard with all its text
/// selected, as a field that asks for its old text selected has once it is laid out (a key typed
/// before then is selected and typed over), or panics with the page.
pub(super) fn wait_selected_all(h: &mut Harness, selector: &str) {
    let start = Instant::now();
    loop {
        let ready = h.focus_of(selector) == ds_harness::FocusState::Focused
            && h.with_doc(|doc| {
                let id = doc.query_selector(selector).ok().flatten()?;
                let input = doc.get_node(id)?.element_data()?.text_input_data()?;
                let text = input.editor.raw_text();
                Some(!text.is_empty() && input.editor.selected_text() == Some(text))
            }) == Some(true);
        if ready {
            return;
        }
        h.advance(STEP);
        assert!(
            start.elapsed() < LIMIT,
            "timed out waiting for {selector} to select its text:\n{}",
            h.html()
        );
    }
}
