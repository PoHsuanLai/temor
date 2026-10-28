//! The sidebar in ds-harness: its controls never lie over one another (the closed Downloads
//! disclosure once laid out its hint over the foot's buttons); every row is one line, a split tab
//! is Arc's row of side-by-side panes that close from their menu, Downloads ends the head, and
//! the Space name and dots show only once there are two Spaces.

use std::time::{Duration, Instant};

use ds::base::press::PointerButton;
use ds::base::vocab::ShortcutKey;
use ds::prelude::{Point, Px};
use ds_blitz::Extent;
use ds_harness::{Harness, Input};
use temor_ui::view::probe::Probe;

use ds::components::app::spaces::{SpaceId, Spaces};
use temor_client::keep::older::save_spaces;
use temor_client::model::space::Project;

use super::drive;
use super::drive::{chord, wait_until};
use super::window::{harness, harness_in};

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
    if !super::support::window_tools_present() {
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
    // With one Space its name still heads the sidebar (it says where you are), but there is no
    // dot to switch to. Downloads stands at the head's right end.
    assert_eq!(
        rects(&h, ".temor-side .temor-head .ds-space-head").len(),
        1,
        "the Space's name"
    );
    assert!(
        rects(&h, ".temor-side .ds-spaces-dots").is_empty(),
        "no lone dot"
    );
    let head = &rects(&h, ".temor-side .temor-head")[0];
    let downloads = &rects(&h, ".temor-head .temor-dl-hold")[0];
    assert!(
        (head.right - downloads.right).abs() <= 1.0,
        "Downloads ends the head: {} and {}",
        downloads.right,
        head.right
    );
    // Back and Forward stand before Downloads, with nowhere to go yet; the address of the row in
    // front is under the head.
    let walk = &rects(&h, ".temor-head .temor-walk")[0];
    assert!(
        walk.right <= downloads.left,
        "Back and Forward, then Downloads"
    );
    assert_eq!(
        rects(&h, ".temor-head .temor-walk .ds-button[disabled]").len(),
        2,
        "nowhere to go"
    );
    let address = rects(&h, ".temor-side .temor-address .ds-path-segment");
    assert!(!address.is_empty(), "the shell's folder");
    assert!(address[0].top >= head.bottom, "under the head");
    // The foot has no menu of its own: New Space is in the Space's menu.
    assert!(
        rects(&h, ".temor-side .temor-foot .ds-button").is_empty(),
        "no chevron"
    );
    // A split's panes close from their menu, never from a button on hover.
    assert!(rects(&h, ".temor-seg-close").is_empty(), "no hover close");
}

/// With two Spaces the head names the one on screen, to the left of Downloads, and the foot's
/// dots stand at its centre.
#[test]
fn two_spaces_show_their_name_and_dots() {
    if !super::support::window_tools_present() {
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

/// A right click on the sidebar's bare part is the Space's menu, as mailo's is; on a tab it is the
/// tab's own menu alone.
#[test]
fn a_right_click_on_the_bare_sidebar_opens_the_space_s_menu_and_on_a_tab_the_tab_s() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness("sidebar-space-menu", &probe);
    h.resize_window(Extent::new(1280, 800));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    let tab = &rects(&h, ".temor-side .temor-tab-box")[0];
    let side = &rects(&h, ".temor-side")[0];
    // Below the rows, above the foot: nothing there has a menu of its own.
    let bare = Point {
        x: Px(((side.left + side.right) / 2.0) as f32),
        y: Px(((tab.bottom + side.bottom) / 2.0) as f32),
    };
    h.send(Input::press(bare, PointerButton::Secondary));
    h.advance(Duration::from_millis(300));
    assert!(
        h.html().contains("Edit Space\u{2026}"),
        "the Space's menu:\n{}",
        h.html()
    );
    assert!(!h.html().contains("Pin Tab"), "only the Space's menu");
    h.send(Input::key(ShortcutKey::Escape));
    h.advance(Duration::from_millis(300));
    assert!(
        !h.html().contains("Edit Space\u{2026}"),
        "closed:\n{}",
        h.html()
    );

    let on_tab = Point {
        x: Px(((tab.left + tab.right) / 2.0) as f32),
        y: Px(((tab.top + tab.bottom) / 2.0) as f32),
    };
    h.send(Input::press(on_tab, PointerButton::Secondary));
    h.advance(Duration::from_millis(300));
    assert!(
        h.html().contains("Pin Tab"),
        "the tab's menu:\n{}",
        h.html()
    );
    assert!(
        !h.html().contains("Edit Space\u{2026}"),
        "not the Space's menu too"
    );
}

/// A right click on a pane of a split is the tab's menu with Close Pane, which closes that pane
/// alone.
#[test]
fn a_split_pane_closes_from_its_menu() {
    if !super::support::window_tools_present() {
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
    let close = super::drive::menu_item(&h, "Close Pane").centre();
    h.send(Input::click(close));
    h.advance(Duration::from_millis(400));
    assert!(
        rects(&h, ".temor-side .temor-seg").is_empty(),
        "one pane left:\n{}",
        h.html()
    );
}

/// A middle press on a pane of a split opens no menu: only a right click does, as on a row.
#[test]
fn a_middle_press_on_a_split_pane_opens_no_menu() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness("sidebar-middle-segment", &probe);
    h.resize_window(Extent::new(1280, 800));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    chord(&mut h, 'd');
    let segment = h
        .centre(".temor-side .temor-seg")
        .unwrap_or_else(|| panic!("no segment:\n{}", h.html()));
    h.send(Input::press(segment, PointerButton::Middle));
    h.advance(Duration::from_millis(300));
    assert!(
        !h.html().contains("Close Pane"),
        "no menu from a middle press:\n{}",
        h.html()
    );
}

/// The boxes of what `selector` finds, in document order.
fn rects(h: &Harness, selector: &str) -> Vec<Box4> {
    drive::rects(h, selector)
        .into_iter()
        .map(|r| Box4 {
            left: r.x,
            top: r.y,
            right: r.right(),
            bottom: r.y + r.h,
            what: selector.to_owned(),
        })
        .collect()
}

#[test]
fn a_hidden_sidebar_peeks_from_the_edge_and_a_click_pins_it() {
    if !super::support::window_tools_present() {
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
        &[ShortcutKey::Ctrl, ShortcutKey::Super],
        ShortcutKey::Char('s'),
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
    // The pin is waited for, not timed: a loaded machine draws it later.
    let start = Instant::now();
    while found(&h, ".temor-edge-host") || !found(&h, ".ds-side[data-side=shown]") {
        h.advance(Duration::from_millis(100));
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "pinned in its column, with no edge host"
        );
    }
}

/// A split of three panes is squeezed as Arc's is: the pane with the keys keeps its name, the
/// others are their glyphs alone.
#[test]
fn a_split_of_three_shows_one_name_and_two_glyphs() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness("sidebar-squeeze", &probe);
    h.resize_window(Extent::new(1700, 800));
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

/// Pinned tabs move as Arc's do: a loose row dropped on a pinned row is pinned, a pinned row
/// dropped on a loose one is unpinned there.
#[test]
fn a_row_dropped_on_a_pinned_row_pins_and_a_pinned_row_dropped_on_a_loose_one_unpins() {
    if !super::support::window_tools_present() {
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
    let pinned = |h: &Harness| rects(h, ".temor-side .temor-tab-box[data-pinned=true]");
    let loose = |h: &Harness| {
        let all = rects(h, ".temor-side .temor-tab-box");
        let pins = pinned(h);
        all.into_iter()
            .filter(|row| !pins.iter().any(|pin| pin.top == row.top))
            .collect::<Vec<_>>()
    };
    let counts = |h: &Harness| (loose(h).len(), pinned(h).len());
    assert_eq!(counts(&h), (3, 0), "three tabs");
    // Pin Tab is in the row's menu.
    let first = centre(&loose(&h)[0]);
    h.send(Input::press(first, PointerButton::Secondary));
    h.advance(Duration::from_millis(300));
    let pin = super::drive::menu_item(&h, "Pin Tab");
    h.send(Input::click(pin.centre()));
    h.advance(Duration::from_millis(400));
    assert_eq!(counts(&h), (2, 1), "pinned from the menu");
    // A loose row onto the pinned one: pinned too.
    let (row, pin) = (centre(&loose(&h)[0]), centre(&pinned(&h)[0]));
    h.send(Input::drag(row, pin, 12));
    h.advance(Duration::from_millis(400));
    assert_eq!(counts(&h), (1, 2), "a row dropped on a pinned row");
    // A pinned row onto the loose one: unpinned.
    let (pin, row) = (centre(&pinned(&h)[0]), centre(&loose(&h)[0]));
    h.send(Input::drag(pin, row, 12));
    h.advance(Duration::from_millis(400));
    assert_eq!(counts(&h), (2, 1), "a pinned row dropped on a loose one");
}

/// A row dragged to the top of the sidebar keeps its folder among the Favourites, as Arc's do; the
/// tile's menu lets it go.
#[test]
fn a_row_dropped_on_the_favourites_keeps_its_folder_there() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness("sidebar-favourite", &probe);
    h.resize_window(Extent::new(1280, 800));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    let centre = |b: &Box4| ds::prelude::Point {
        x: ds::prelude::Px(((b.left + b.right) / 2.0) as f32),
        y: ds::prelude::Px(((b.top + b.bottom) / 2.0) as f32),
    };
    let tiles = |h: &Harness| rects(h, ".temor-side .temor-fav");
    // None kept: they take no room until a drag shows where to drop one.
    assert!(
        rects(&h, ".temor-favs-drop").is_empty(),
        "no drop box at rest"
    );
    let row = centre(&rects(&h, ".temor-side .temor-tab-box")[0]);
    let first_top = rects(&h, ".temor-side .temor-tab-box")[0].top;
    h.send(Input::pointer_down(row));
    for step in 1..=6 {
        let mut at = row;
        at.y.0 -= step as f32 * 3.0;
        h.send(Input::pointer_move(at));
        h.advance(Duration::from_millis(16));
    }
    h.advance(Duration::from_millis(100));
    let zone = rects(&h, ".temor-favs-drop");
    assert_eq!(zone.len(), 1, "a drag shows the drop box:\n{}", h.html());
    assert_eq!(
        rects(&h, ".temor-side .temor-tab-box")[0].top,
        first_top,
        "the rows do not move under the drag"
    );
    let over = centre(&zone[0]);
    h.send(Input::pointer_move(over));
    h.advance(Duration::from_millis(16));
    h.send(Input::pointer_up(over));
    h.advance(Duration::from_millis(400));
    assert!(rects(&h, ".temor-favs-drop").is_empty(), "the box goes");
    assert_eq!(tiles(&h).len(), 1, "kept:\n{}", h.html());
    // Its menu lets it go.
    h.send(Input::press(
        centre(&tiles(&h)[0]),
        PointerButton::Secondary,
    ));
    h.advance(Duration::from_millis(300));
    let remove = super::drive::menu_item(&h, "Remove from Favourites");
    h.send(Input::click(remove.centre()));
    h.advance(Duration::from_millis(400));
    assert!(tiles(&h).is_empty(), "let go:\n{}", h.html());
}

/// ⌘-pressing a row (Ctrl off a Mac) opens a new tab in its folder, as a browser's link opens
/// beside the page; a plain press only brings the tab to the front.
#[test]
fn a_command_press_on_a_row_opens_a_new_tab_beside_it() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness("sidebar-command-press", &probe);
    h.resize_window(Extent::new(1280, 800));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    let first = &rects(&h, ".temor-side .temor-tab-box")[0];
    let row = Point {
        x: Px(((first.left + first.right) / 2.0) as f32),
        y: Px(((first.top + first.bottom) / 2.0) as f32),
    };
    h.send(Input::click(row));
    h.advance(Duration::from_millis(300));
    assert_eq!(drive::rows(&h), 1, "a press opens nothing");
    let command = match cfg!(target_os = "macos") {
        true => dioxus::prelude::Modifiers::SUPER,
        false => dioxus::prelude::Modifiers::CONTROL,
    };
    h.send(Input::Pointer(
        ds_harness::PointerInput::new(
            row,
            ds_harness::PointerAction::Click(PointerButton::Primary),
        )
        .with_mods(command),
    ));
    h.advance(Duration::from_millis(400));
    assert_eq!(drive::rows(&h), 2, "a new tab:\n{}", h.html());
}

/// A row's menu is the one menu: open, keep, show, close, in that order; Add to Favourites keeps
/// its folder as a tile.
#[test]
fn a_rows_menu_is_the_one_menu_and_keeps_its_folder_among_the_favourites() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness("sidebar-row-menu", &probe);
    h.resize_window(Extent::new(1280, 800));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    let row = &rects(&h, ".temor-side .temor-tab-box")[0];
    let at = ds::prelude::Point {
        x: ds::prelude::Px(((row.left + row.right) / 2.0) as f32),
        y: ds::prelude::Px(((row.top + row.bottom) / 2.0) as f32),
    };
    h.send(Input::press(at, PointerButton::Secondary));
    h.advance(Duration::from_millis(300));
    let titles: Vec<String> = h.with_doc(|doc| {
        doc.query_selector_all(".ds-menu-item")
            .map(|found| found.into_iter().collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|id| Some(doc.get_node(id)?.text_content().trim().to_owned()))
            .collect()
    });
    assert_eq!(
        titles,
        [
            "Open in New Tab",
            "Add to Favourites",
            "Pin Tab",
            "Move to Folder",
            "Show in Folder",
            "Show History",
            "Close Tab"
        ]
    );
    let keep = drive::menu_item(&h, "Add to Favourites");
    h.send(Input::click(keep.centre()));
    let start = Instant::now();
    while rects(&h, ".temor-side .temor-fav").is_empty() {
        h.advance(Duration::from_millis(25));
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "no tile:\n{}",
            h.html()
        );
    }
}

/// A row is led by what runs in its tab: the letter tile of its folder at the prompt, and a
/// program's brand while it runs (bash is Bash's), back to the letter once it ends.
#[test]
fn a_row_shows_its_folders_letter_and_a_running_programs_brand() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness("sidebar-icons", &probe);
    h.resize_window(Extent::new(1280, 800));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    // The one row's `data-leading`: what leads it.
    let leading = |h: &Harness| -> (usize, Option<String>) {
        let row = ".temor-side .temor-tab-box";
        let leads = ".temor-side .temor-tab-box [data-leading]";
        (h.count(row), h.attr(leads, "data-leading"))
    };
    let settle = |h: &mut Harness, want: &str| {
        let start = Instant::now();
        while leading(h) != (1, Some(want.to_owned())) {
            h.advance(Duration::from_millis(25));
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "not {want}: {:?}\n{}",
                leading(h),
                h.html()
            );
        }
    };
    settle(&mut h, "avatar");
    drive::type_line(&mut h, "bash -c 'sleep 30'");
    settle(&mut h, "element");
    assert_eq!(rects(&h, ".temor-side .temor-tab-brand").len(), 1);
    drive::chord_with(&mut h, &[ShortcutKey::Ctrl], 'c');
    settle(&mut h, "avatar");
}

/// At its prompt a row shows the icon Finder set on its folder, found off the window's thread,
/// and its letter again in a folder without one.
#[cfg(target_os = "macos")]
#[test]
fn a_row_at_its_prompt_shows_the_icon_set_on_its_folder() {
    if !super::support::window_tools_present() {
        return;
    }
    let dir = tempfile::tempdir().expect("dir");
    let png = temor_client::icons::app_png(
        std::path::Path::new("/System/Applications/Calculator.app"),
        32,
    )
    .expect("a PNG to put in the icon");
    let icon = dir.path().join("Icon\r");
    std::fs::write(&icon, "").expect("Icon file");
    std::fs::write(icon.join("..namedfork/rsrc"), fork_holding(&icns_of(&png))).expect("fork");
    let probe = Probe::new();
    let mut h = harness("sidebar-folder-icon", &probe);
    h.resize_window(Extent::new(1280, 800));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    let app = ".temor-side .temor-tab-box .temor-tab-app";
    drive::type_line(&mut h, &format!("cd '{}'", dir.path().display()));
    let start = Instant::now();
    while h.count(app) != 1 {
        h.advance(Duration::from_millis(25));
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "no folder icon:\n{}",
            h.html()
        );
    }
    drive::type_line(&mut h, "cd /");
    let start = Instant::now();
    while h.count(app) != 0 {
        h.advance(Duration::from_millis(25));
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "the icon stayed:\n{}",
            h.html()
        );
    }
}

/// An icon file holding `png`.
#[cfg(target_os = "macos")]
fn icns_of(png: &[u8]) -> Vec<u8> {
    let mut icns = b"icns".to_vec();
    icns.extend_from_slice(&(16 + png.len() as u32).to_be_bytes());
    icns.extend_from_slice(b"ic11");
    icns.extend_from_slice(&(8 + png.len() as u32).to_be_bytes());
    icns.extend_from_slice(png);
    icns
}

/// A resource fork with one `icns` resource, as Finder writes a folder's custom icon.
#[cfg(target_os = "macos")]
fn fork_holding(icns: &[u8]) -> Vec<u8> {
    let body: Vec<u8> = [(icns.len() as u32).to_be_bytes().as_slice(), icns].concat();
    let (data_at, map_at) = (256u32, 256 + body.len() as u32);
    let mut map = vec![0u8; 24];
    for half in [28u16, 0, 0] {
        map.extend_from_slice(&half.to_be_bytes());
    }
    map.extend_from_slice(b"icns");
    map.extend_from_slice(&0u16.to_be_bytes());
    map.extend_from_slice(&10u16.to_be_bytes());
    map.extend_from_slice(&(-16455i16).to_be_bytes());
    map.extend_from_slice(&0xFFFFu16.to_be_bytes());
    map.extend_from_slice(&[0; 8]);
    let mut fork: Vec<u8> = [data_at, map_at, body.len() as u32, map.len() as u32]
        .iter()
        .flat_map(|n| n.to_be_bytes())
        .collect();
    fork.resize(data_at as usize, 0);
    fork.extend_from_slice(&body);
    fork.extend_from_slice(&map);
    fork
}
