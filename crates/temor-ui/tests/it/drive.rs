//! Driving a window from a test: typing, chords, clicks, waiting, and measuring boxes. Every
//! file that drives a window uses these, so the settle times and time limits are the same
//! everywhere.

use std::time::{Duration, Instant};

use ds::base::vocab::ShortcutKey;
use ds::prelude::{Point, Px};
use ds_harness::{Harness, Input};
use temor_ui::view::probe::Probe;

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

/// Types `line`, and waits until `last` is a line on the screen (the command's output).
pub(super) fn run_line(h: &mut Harness, probe: &Probe, line: &str, last: &str) {
    type_line(h, line);
    wait_until(h, probe, "the command's output", |s| {
        s.lines().any(|l| l.trim_end() == last)
    });
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

/// Clicks the middle of the element `selector` names; panics, with the page, if there is none.
pub(super) fn press(h: &mut Harness, selector: &str) {
    let at = h
        .centre(selector)
        .unwrap_or_else(|| panic!("nothing matches {selector}:\n{}", h.html()));
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
            "timed out waiting for {what}:\n{}",
            probe.screen()
        );
    }
}

/// A window that shows its first prompt.
pub(super) fn started(name: &str, probe: &Probe) -> Harness {
    let mut h = harness(name, probe);
    wait_until(&mut h, probe, "the prompt", |s| s.contains('$'));
    h
}

/// A window whose pane has the keyboard and shows its first prompt.
pub(super) fn ready(name: &str, probe: &Probe) -> Harness {
    let mut h = started(name, probe);
    focus_pane(&mut h);
    h
}

/// How many tab rows the sidebar shows.
pub(super) fn rows(h: &Harness) -> usize {
    let html = h.html();
    html.matches("class=\"temor-tab-box\"").count() + html.matches("class=\"temor-pin\"").count()
}

/// How many terminal panes the tab in front shows.
pub(super) fn panes(h: &Harness) -> usize {
    h.html().matches("aria-label=\"Terminal\"").count()
}

/// A box: left, top, width, height.
#[derive(Debug, Clone, Copy)]
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
