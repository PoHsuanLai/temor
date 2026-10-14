//! The sidebar in ds-harness: its controls never lie over one another (the closed Downloads
//! disclosure once laid out its hint over the foot's buttons); every row is one line, a split tab
//! is Arc's row of side-by-side panes that close from their menu, Downloads ends the head, and
//! the Space name and dots show only once there are two Spaces.

use std::time::Duration;

use ds::base::press::PointerButton;
use ds::base::vocab::ShortcutKey;
use ds_blitz::Extent;
use ds_harness::{DocQuery, Driver, Harness, Input, Query};
use temor_ui::view::probe::Probe;

use ds::components::app::spaces::{SpaceId, Spaces};
use temor_core::space::Project;
use temor_ui::edge::save_spaces;

use super::window::{harness, harness_in, wait_until};

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
    // With one Space there is no Space to name or switch to: no head name and no dot. Downloads
    // stands at the head's right end, the chevron at the foot's.
    assert!(
        rects(&h, ".temor-side .ds-space-head").is_empty(),
        "no Space name"
    );
    assert!(
        rects(&h, ".temor-side .ds-spaces-dots").is_empty(),
        "no lone dot"
    );
    let side = &rects(&h, ".temor-side")[0];
    let head = &rects(&h, ".temor-side .temor-head")[0];
    let downloads = &rects(&h, ".temor-head .temor-dl-hold")[0];
    let chevron = &rects(&h, ".temor-side .temor-foot-end .ds-button")[0];
    assert!(
        (head.right - downloads.right).abs() <= 1.0,
        "Downloads ends the head: {} and {}",
        downloads.right,
        head.right
    );
    assert!(
        (downloads.right - chevron.right).abs() <= 1.0,
        "Downloads stands over the chevron: {} and {}",
        downloads.right,
        chevron.right
    );
    assert!(
        chevron.top > side.top + (side.bottom - side.top) / 2.0,
        "the chevron is at the foot"
    );
    // A split's panes close from their menu, never from a button on hover.
    assert!(rects(&h, ".temor-seg-close").is_empty(), "no hover close");
}

/// With two Spaces the head names the one on screen, to the left of Downloads, and the foot's
/// dots stand at its centre.
#[test]
fn two_spaces_show_their_name_and_dots() {
    if !super::support::tools_present() {
        return;
    }
    let state = super::support::scratch("sidebar-two-spaces-state").join("state");
    let mut spaces = Spaces::first_run([Project::default(), Project::default()], Project::default);
    spaces.rename(SpaceId(0), "Home");
    spaces.rename(SpaceId(1), "Work");
    save_spaces(&state, &spaces).expect("spaces saved");
    let probe = Probe::new();
    let mut h = harness_in("sidebar-two-spaces", &probe, Some(state));
    h.resize_window(Extent::new(1280, 800));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    assert_eq!(
        h.text_of(".ds-space-head").unwrap_or_default().trim(),
        "Home"
    );
    let name = &rects(&h, ".temor-head .ds-space-head")[0];
    let downloads = &rects(&h, ".temor-head .temor-dl-hold")[0];
    assert!(name.right <= downloads.left, "the name, then Downloads");
    let foot = rects(&h, ".temor-side .ds-spaces-foot");
    let dots = rects(&h, ".temor-side .ds-spaces-dots");
    let (foot, dots) = (&foot[0], &dots[0]);
    let off = ((dots.left + dots.right) - (foot.left + foot.right)).abs() / 2.0;
    assert!(off <= 1.0, "the dots are {off} px off the foot's centre");
}

/// A right click on a pane of a split is the tab's menu with Close Pane, which closes that pane
/// alone.
#[test]
fn a_split_pane_closes_from_its_menu() {
    if !super::support::tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness("sidebar-close-pane", &probe);
    h.resize_window(Extent::new(1280, 800));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    chord(&mut h, 'd');
    assert_eq!(rects(&h, ".temor-side .temor-seg").len(), 2, "two panes");
    let segment = h
        .centre(".temor-side .temor-seg")
        .unwrap_or_else(|| panic!("no segment:\n{}", h.html()));
    h.send(Input::press(segment, PointerButton::Secondary));
    h.advance(Duration::from_millis(300));
    assert!(
        h.html().contains("Close Pane"),
        "the segment's menu:\n{}",
        h.html()
    );
    // Pin Tab, then Close Pane.
    let item = rects(&h, ".ds-menu-item");
    assert!(item.len() >= 2, "the segment's menu opens:\n{}", h.html());
    let close = ds::prelude::Point {
        x: ds::prelude::Px(((item[1].left + item[1].right) / 2.0) as f32),
        y: ds::prelude::Px(((item[1].top + item[1].bottom) / 2.0) as f32),
    };
    h.send(Input::click(close));
    h.advance(Duration::from_millis(400));
    assert!(
        rects(&h, ".temor-side .temor-seg").is_empty(),
        "one pane left:\n{}",
        h.html()
    );
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

/// Hidden, the sidebar waits at the window's edge: the pointer there floats it over the pane, as
/// mailo's does, and a click on the edge pins it again.
#[test]
fn a_hidden_sidebar_peeks_from_the_edge_and_a_click_pins_it() {
    if !super::support::tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness("sidebar-peek", &probe);
    h.resize_window(Extent::new(1280, 800));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    let at = |x: f32, y: f32| ds::prelude::Point {
        x: ds::prelude::Px(x),
        y: ds::prelude::Px(y),
    };
    h.send(Input::click(at(700.0, 300.0)));
    h.advance(Duration::from_millis(200));
    let found = |h: &Harness, selector: &str| {
        h.with_doc(|doc| doc.query_selector(selector).ok().flatten().is_some())
    };
    h.send(Input::chord(
        &[ShortcutKey::Super, ShortcutKey::Shift],
        ShortcutKey::Char('l'),
    ));
    h.advance(Duration::from_millis(600));
    assert!(
        found(&h, ".temor-edge-host .ds-side[data-side=hidden]"),
        "hidden at the edge"
    );
    h.send(Input::pointer_move(at(3.0, 400.0)));
    h.advance(Duration::from_millis(900));
    assert!(
        found(&h, ".temor-edge-host .ds-side[data-side=peek]"),
        "the edge peeks it"
    );
    let side = rects(&h, ".temor-edge-host .ds-side");
    assert!(
        side[0].right - side[0].left > 200.0,
        "it floats at its width"
    );
    // Away from it, it slides back out; a click on the edge pins it.
    h.send(Input::pointer_move(at(900.0, 400.0)));
    h.advance(Duration::from_millis(900));
    assert!(
        found(&h, ".temor-edge-host .ds-side[data-side=hidden]"),
        "gone again"
    );
    h.send(Input::click(at(3.0, 400.0)));
    h.advance(Duration::from_millis(600));
    assert!(!found(&h, ".temor-edge-host"), "pinned: no edge host");
    assert!(
        found(&h, ".ds-side[data-side=shown]"),
        "pinned in its column"
    );
}

/// A split of three panes is squeezed as Arc's is: the pane with the keys keeps its name, the
/// others are their glyphs alone.
#[test]
fn a_split_of_three_shows_one_name_and_two_glyphs() {
    if !super::support::tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness("sidebar-squeeze", &probe);
    h.resize_window(Extent::new(1600, 800));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    chord(&mut h, 'd');
    chord(&mut h, 'd');
    let slots = rects(&h, ".temor-side .temor-seg-slot");
    assert_eq!(slots.len(), 3, "three panes");
    let squeezed = rects(&h, ".temor-side .temor-seg-slot[data-glyph=true]");
    assert_eq!(squeezed.len(), 2, "two are glyphs");
    for slot in &squeezed {
        assert!(
            slot.right - slot.left < 40.0,
            "a glyph is narrow: {}",
            slot.right - slot.left
        );
    }
    let names = rects(
        &h,
        ".temor-side .temor-seg-slot[data-glyph=true] .temor-seg-name",
    );
    assert!(
        names.iter().all(|r| r.right - r.left < 0.5),
        "no names on the glyphs"
    );
}

/// Pinned tabs move as Arc's do: a row dropped on a pinned tile is pinned, a tile dropped on a row
/// is unpinned there.
#[test]
fn a_row_dropped_on_a_tile_pins_and_a_tile_dropped_on_a_row_unpins() {
    if !super::support::tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness("sidebar-pin-drag", &probe);
    h.resize_window(Extent::new(1280, 800));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    chord(&mut h, 'n');
    chord(&mut h, 'n');
    let centre = |b: &Box4| ds::prelude::Point {
        x: ds::prelude::Px(((b.left + b.right) / 2.0) as f32),
        y: ds::prelude::Px(((b.top + b.bottom) / 2.0) as f32),
    };
    let rows = |h: &Harness| rects(h, ".temor-side .temor-tab-box");
    let tiles = |h: &Harness| rects(h, ".temor-side .temor-pin");
    assert_eq!((rows(&h).len(), tiles(&h).len()), (3, 0), "three tabs");
    // Pin Tab heads the row's menu.
    let first = centre(&rows(&h)[0]);
    h.send(Input::press(first, PointerButton::Secondary));
    h.advance(Duration::from_millis(300));
    let item = rects(&h, ".ds-menu-item");
    assert!(!item.is_empty(), "the row's menu opens");
    h.send(Input::click(centre(&item[0])));
    h.advance(Duration::from_millis(400));
    assert_eq!(
        (rows(&h).len(), tiles(&h).len()),
        (2, 1),
        "pinned from the menu"
    );
    // A row onto the tile: pinned too.
    let (row, tile) = (centre(&rows(&h)[0]), centre(&tiles(&h)[0]));
    h.send(Input::drag(row, tile, 12));
    h.advance(Duration::from_millis(400));
    assert_eq!(
        (rows(&h).len(), tiles(&h).len()),
        (1, 2),
        "a row dropped on a tile"
    );
    // A tile onto the row: unpinned.
    let (tile, row) = (centre(&tiles(&h)[0]), centre(&rows(&h)[0]));
    h.send(Input::drag(tile, row, 12));
    h.advance(Duration::from_millis(400));
    assert_eq!(
        (rows(&h).len(), tiles(&h).len()),
        (2, 1),
        "a tile dropped on a row"
    );
}
