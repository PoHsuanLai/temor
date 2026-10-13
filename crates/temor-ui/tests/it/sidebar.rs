//! The sidebar in ds-harness: its controls never lie over one another (the closed Downloads
//! disclosure once laid out its hint over the foot's buttons); every row is one line, a split tab
//! is Arc's row of side-by-side panes, and the Space dots stand at the foot's centre.

use std::time::Duration;

use ds::base::vocab::ShortcutKey;
use ds_blitz::Extent;
use ds_harness::{DocQuery, Driver, Harness, Input};
use temor_ui::view::probe::Probe;

use super::window::{harness, wait_until};

fn chord(h: &mut Harness, key: char) {
    h.send(Input::chord(
        &[ShortcutKey::Ctrl, ShortcutKey::Shift],
        ShortcutKey::Char(key),
    ));
    h.advance(Duration::from_millis(400));
}

/// A box in the window: left, top, right, bottom, and what it is.
struct Box4 {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
    what: String,
}

impl Box4 {
    fn contains(&self, other: &Box4) -> bool {
        self.left <= other.left
            && self.top <= other.top
            && self.right >= other.right
            && self.bottom >= other.bottom
    }

    /// Whether the two share area and neither holds the other.
    fn lies_over(&self, other: &Box4) -> bool {
        let across = self.left.max(other.left) < self.right.min(other.right) - 0.5
            && self.top.max(other.top) < self.bottom.min(other.bottom) - 0.5;
        across && !self.contains(other) && !other.contains(self)
    }
}

/// The boxes of the sidebar's rows and buttons.
fn controls(h: &Harness) -> Vec<Box4> {
    h.with_doc(|doc| {
        let found = doc
            .query_selector_all(".temor-side .ds-row, .temor-side button")
            .map(|found| found.into_iter().collect::<Vec<_>>())
            .unwrap_or_default();
        found
            .into_iter()
            .filter_map(|id| {
                let rect = doc.get_client_bounding_rect(id)?;
                let what = doc.get_node(id)?.text_content();
                (rect.width > 0.5 && rect.height > 0.5).then(|| Box4 {
                    left: rect.x,
                    top: rect.y,
                    right: rect.x + rect.width,
                    bottom: rect.y + rect.height,
                    what: what.trim().chars().take(24).collect(),
                })
            })
            .collect()
    })
}

fn assert_apart(h: &Harness, state: &str) {
    let boxes = controls(h);
    assert!(boxes.len() >= 3, "{state}: the sidebar has controls");
    for (i, a) in boxes.iter().enumerate() {
        for b in &boxes[i + 1..] {
            assert!(
                !a.lies_over(b),
                "{state}: {:?} ({},{})-({},{}) lies over {:?} ({},{})-({},{})",
                a.what,
                a.left,
                a.top,
                a.right,
                a.bottom,
                b.what,
                b.left,
                b.top,
                b.right,
                b.bottom
            );
        }
    }
}

#[test]
fn no_two_controls_in_the_sidebar_lie_over_one_another() {
    if !super::support::tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness("sidebar-apart", &probe);
    h.resize_window(Extent::new(1280, 800));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    assert_apart(&h, "one tab");
    chord(&mut h, 'n');
    chord(&mut h, 'd');
    assert_apart(&h, "two tabs, one split");
    // Every row is one line, on the sidebar ladder's 32, split or not.
    let heights: Vec<f64> = rects(&h, ".temor-side .temor-tab-box .ds-row")
        .iter()
        .map(|r| r.bottom - r.top)
        .collect();
    assert_eq!(heights.len(), 2, "{heights:?}");
    assert!(
        heights.iter().all(|height| (*height - 32.0).abs() < 0.5),
        "{heights:?}"
    );
    // The split tab is Arc's: its two panes stand side by side in the one row, as wide as each
    // other, each with its name.
    let segments = rects(&h, ".temor-side .temor-seg");
    assert_eq!(segments.len(), 2, "two panes");
    let (a, b) = (&segments[0], &segments[1]);
    assert!(
        (a.top - b.top).abs() < 0.5,
        "one row: {} and {}",
        a.top,
        b.top
    );
    assert!(
        a.right <= b.left,
        "side by side: {} then {}",
        a.right,
        b.left
    );
    let (wa, wb) = (a.right - a.left, b.right - b.left);
    assert!(wa > 40.0 && (wa - wb).abs() < 2.0, "even: {wa} and {wb}");
    // The Space dots stand at the foot's centre, Downloads at its start and the chevron at its end.
    let foot = rects(&h, ".temor-side .ds-spaces-foot");
    let dots = rects(&h, ".temor-side .ds-spaces-dots");
    let (foot, dots) = (&foot[0], &dots[0]);
    let off = ((dots.left + dots.right) - (foot.left + foot.right)).abs() / 2.0;
    assert!(off <= 1.0, "the dots are {off} px off the foot's centre");
}

/// The boxes of what `selector` finds, in document order.
fn rects(h: &Harness, selector: &str) -> Vec<Box4> {
    h.with_doc(|doc| {
        doc.query_selector_all(selector)
            .map(|found| found.into_iter().collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|id| {
                let r = doc.get_client_bounding_rect(id)?;
                Some(Box4 {
                    left: r.x,
                    top: r.y,
                    right: r.x + r.width,
                    bottom: r.y + r.height,
                    what: selector.to_owned(),
                })
            })
            .collect()
    })
}

#[test]
fn a_running_tab_counts_up_with_no_output() {
    if !super::support::tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness("sidebar-running-ticks", &probe);
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    h.send(Input::click(ds::prelude::Point {
        x: ds::prelude::Px(700.0),
        y: ds::prelude::Px(300.0),
    }));
    for c in "sleep 30".chars() {
        let key = match c {
            ' ' => ShortcutKey::Space,
            c => ShortcutKey::Char(c),
        };
        h.send(Input::key(key));
    }
    h.send(Input::key(ShortcutKey::Enter));
    let start = std::time::Instant::now();
    // `sleep` prints nothing: only the timer can move the row's time from 0s or 1s to 3s.
    let time = |h: &Harness| {
        h.with_doc(|doc| {
            doc.query_selector(".temor-tab-time")
                .ok()
                .flatten()
                .and_then(|id| doc.get_node(id).map(|node| node.text_content()))
        })
    };
    while time(&h).as_deref().map(str::trim) != Some("3s") {
        h.advance(Duration::from_millis(50));
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "the tab's time did not count up: {:?}",
            time(&h)
        );
    }
    h.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('c')));
}
