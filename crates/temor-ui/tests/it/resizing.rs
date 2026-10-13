//! The window pushed by size (a break-it pass): a divider dragged and kept through a tab switch,
//! dragged past both ends, a sweep of window sizes with every control kept inside the window,
//! and the pane still taking typing afterwards.

use std::time::Duration;

use ds::base::vocab::ShortcutKey;
use ds::prelude::{Point, Px};
use ds_blitz::Extent;
use ds_harness::{DocQuery, Driver, Harness, Input};
use temor_ui::view::probe::Probe;

use super::window::{harness, wait_until};

/// The panes' split view (the sidebar has one of its own).
const PANES: &str = ".ds-split[aria-label=\"Panes\"]";

/// A box: left, top, width, height.
#[derive(Debug, Clone, Copy)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Rect {
    fn right(self) -> f64 {
        self.x + self.w
    }

    fn centre(self) -> Point {
        Point {
            x: Px((self.x + self.w / 2.0) as f32),
            y: Px((self.y + self.h / 2.0) as f32),
        }
    }
}

fn rects(h: &Harness, selector: &str) -> Vec<Rect> {
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

fn rect(h: &Harness, selector: &str) -> Rect {
    let found = rects(h, selector);
    assert_eq!(found.len(), 1, "one {selector}: {found:?}");
    found[0]
}

fn chord(h: &mut Harness, key: char) {
    h.send(Input::chord(
        &[ShortcutKey::Ctrl, ShortcutKey::Shift],
        ShortcutKey::Char(key),
    ));
    h.advance(Duration::from_millis(400));
}

/// Types `echo <word>$((40+2))` and waits for `<word>42`, which only the output holds.
fn answers(h: &mut Harness, probe: &Probe, word: &str) {
    for c in format!("echo {word}$((40+2))").chars() {
        let key = match c {
            ' ' => ShortcutKey::Space,
            c => ShortcutKey::Char(c),
        };
        h.send(Input::key(key));
    }
    h.send(Input::key(ShortcutKey::Enter));
    let want = format!("{word}42");
    wait_until(h, probe, &want, |s| s.contains(&want));
}

/// A window with its first pane split side by side, the right pane focused.
fn split_window(name: &str, probe: &Probe) -> Harness {
    let mut h = harness(name, probe);
    wait_until(&mut h, probe, "the prompt", |s| s.contains('$'));
    h.send(Input::click(Point {
        x: Px(700.0),
        y: Px(300.0),
    }));
    h.advance(Duration::from_millis(100));
    chord(&mut h, 'd');
    wait_until(&mut h, probe, "the second prompt", |s| s.contains('$'));
    h
}

fn first_pane(h: &Harness) -> Rect {
    rect(h, &format!("{PANES} > .ds-split-pane"))
}

fn drag_divider_to(h: &mut Harness, x: f32) {
    let from = rect(h, &format!("{PANES} > .ds-split-divider")).centre();
    h.send(Input::drag(
        from,
        Point {
            x: Px(x),
            y: from.y,
        },
        12,
    ));
    h.advance(Duration::from_millis(400));
}

#[test]
fn a_dragged_divider_keeps_its_place_through_a_tab_switch() {
    if !super::support::tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = split_window("resize-divider-kept", &probe);
    let before = first_pane(&h);
    let divider = rect(&h, &format!("{PANES} > .ds-split-divider")).centre();
    drag_divider_to(&mut h, divider.x.0 + 150.0);
    let dragged = first_pane(&h);
    assert!(
        (dragged.w - before.w - 150.0).abs() < 4.0,
        "the pane follows the pointer: {before:?} → {dragged:?}"
    );
    // A new tab draws its own tree; coming back mounts this tab's split again from the tree.
    chord(&mut h, 'n');
    assert!(rects(&h, PANES).is_empty(), "the new tab is not split");
    let row = rects(&h, ".temor-side .temor-tab-box .ds-row")[0].centre();
    h.send(Input::click(row));
    h.advance(Duration::from_millis(400));
    let back = first_pane(&h);
    assert!(
        (back.w - dragged.w).abs() < 4.0,
        "the dragged width is kept: {dragged:?} → {back:?}"
    );
}

#[test]
fn a_divider_dragged_past_either_end_stops_at_its_limit() {
    if !super::support::tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = split_window("resize-divider-ends", &probe);
    let split = rect(&h, PANES);
    for (name, to) in [("far left", 0.0), ("far right", 1023.0), ("back", 600.0)] {
        drag_divider_to(&mut h, to);
        let pane = first_pane(&h);
        let rest = rect(&h, &format!("{PANES} > .ds-split-rest"));
        let least = split.w * 0.1 - 1.0;
        assert!(
            pane.w >= least && rest.w >= least,
            "{name}: each side keeps a tenth of {split:?}: {pane:?} {rest:?}"
        );
        assert!(
            (pane.x - split.x).abs() < 1.0 && (rest.right() - split.right()).abs() < 1.0,
            "{name}: the panes fill the split: {split:?} {pane:?} {rest:?}"
        );
        assert!(
            pane.right() <= rest.x + 1.0,
            "{name}: the panes do not overlap: {pane:?} {rest:?}"
        );
    }
    answers(&mut h, &probe, "ends");
}

#[test]
fn every_control_stays_inside_the_window_at_every_size() {
    if !super::support::tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = split_window("resize-sweep", &probe);
    chord(&mut h, 'n');
    let controls = "button, [role=\"button\"], input, .ds-row";
    let mut wrong = Vec::new();
    for (width, height) in [
        (400, 360),
        (600, 480),
        (760, 520),
        (1000, 640),
        (1280, 800),
        (1920, 1080),
        (400, 1080),
        (1920, 360),
    ] {
        h.resize_window(Extent::new(width, height));
        h.advance(Duration::from_millis(400));
        for found in rects(&h, controls) {
            let shown = found.w >= 0.5 && found.h >= 0.5;
            let outside = found.x < -0.5 || found.right() > f64::from(width) + 0.5;
            if shown && outside {
                wrong.push(format!("{width}x{height}: {found:?}"));
            }
        }
        for pane in rects(&h, ".ds-split-pane, .ds-split-rest") {
            if pane.w < 0.0 || pane.h < 0.0 {
                wrong.push(format!(
                    "{width}x{height}: a pane of negative size {pane:?}"
                ));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "controls outside the window:\n{}",
        wrong.join("\n")
    );
    h.resize_window(Extent::new(1024, 640));
    h.advance(Duration::from_millis(400));
    answers(&mut h, &probe, "sweep");
}

#[test]
fn keys_alone_open_split_move_and_close_without_a_click() {
    if !super::support::tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness("resize-keys-only", &probe);
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    answers(&mut h, &probe, "first");
    chord(&mut h, 'n');
    wait_until(&mut h, &probe, "the new tab's prompt", |s| s.contains('$'));
    answers(&mut h, &probe, "tab");
    chord(&mut h, 'd');
    wait_until(&mut h, &probe, "the split's prompt", |s| s.contains('$'));
    answers(&mut h, &probe, "split");
    // The ⌘T bar opens over the pane; Escape closes it and the keys go back to the pane.
    chord(&mut h, 't');
    h.send(Input::key(ShortcutKey::Escape));
    h.advance(Duration::from_millis(300));
    answers(&mut h, &probe, "bar");
    chord(&mut h, 'w');
    assert!(rects(&h, PANES).is_empty(), "the split closed");
    answers(&mut h, &probe, "closed");
}

#[test]
fn the_bar_stays_inside_the_window_while_it_is_resized_open() {
    if !super::support::tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = split_window("resize-bar-open", &probe);
    chord(&mut h, 't');
    for (width, height) in [(400, 360), (1920, 1080), (600, 300), (1024, 640)] {
        h.resize_window(Extent::new(width, height));
        h.advance(Duration::from_millis(400));
        let bar = rect(&h, ".ds-palette");
        let (w, ht) = (f64::from(width), f64::from(height));
        assert!(
            bar.x >= -0.5 && bar.right() <= w + 0.5 && bar.y >= -0.5 && bar.y + bar.h <= ht + 0.5,
            "{width}x{height}: the bar leaves the window: {bar:?}"
        );
    }
    h.send(Input::key(ShortcutKey::Escape));
    h.advance(Duration::from_millis(300));
    assert!(rects(&h, ".ds-palette").is_empty(), "Escape closed the bar");
    answers(&mut h, &probe, "barsized");
}
