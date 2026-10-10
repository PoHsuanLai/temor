//! The sidebar in ds-harness: its controls never lie over one another (the closed Downloads
//! disclosure once laid out its hint over the foot's buttons), and a row keeps its height whether
//! or not its tab is split.

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
    // The split tab's miniature trails its row: both rows are as tall as the sidebar ladder's 36.
    let heights: Vec<f64> = h.with_doc(|doc| {
        doc.query_selector_all(".temor-side .temor-tab-box .ds-row")
            .map(|found| found.into_iter().collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|id| doc.get_client_bounding_rect(id).map(|r| r.height))
            .collect()
    });
    assert_eq!(heights.len(), 2, "{heights:?}");
    assert!(
        heights.iter().all(|height| (*height - 36.0).abs() < 0.5),
        "{heights:?}"
    );
}
