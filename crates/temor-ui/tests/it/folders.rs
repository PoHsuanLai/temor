//! Folders of tabs in the sidebar, as Arc's are: made from a row's menu and named where they
//! stand, opened and closed with a press, a row dropped on one goes in, and deleting one leaves
//! its tabs.

use std::time::Duration;

use ds::base::press::PointerButton;
use ds::base::vocab::ShortcutKey;
use ds_blitz::Extent;
use ds_harness::{Harness, Input};
use temor_ui::view::probe::Probe;

use super::drive::{self, chord, menu_item, rects, type_text, wait_until};
use super::window::harness;

/// The open menu's item that says `title` is clicked.
fn pick(h: &mut Harness, title: &str) {
    let item = menu_item(h, title);
    h.send(Input::click(item.centre()));
    h.advance(Duration::from_millis(400));
}

/// The rows in a folder.
fn filed(h: &Harness) -> usize {
    rects(h, ".temor-side-folder .temor-tab-box")
        .iter()
        .filter(|row| row.h > 0.0)
        .count()
}

/// The folder's row says `open` (its `aria-expanded`).
fn expanded(h: &Harness) -> Option<String> {
    h.attr(".temor-side-folder .ds-row[aria-expanded]", "aria-expanded")
}

#[test]
fn a_folder_is_made_from_a_rows_menu_named_where_it_stands_opened_filled_and_deleted() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness("sidebar-folders", &probe);
    h.resize_window(Extent::new(1280, 800));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    chord(&mut h, 'n');
    chord(&mut h, 'n');
    assert_eq!(drive::rows(&h), 3, "three tabs");
    // Move to Folder › New Folder: the row goes into a folder whose name is asked for at once.
    let row = rects(&h, ".temor-side .temor-tab-box")[0].centre();
    h.send(Input::press(row, PointerButton::Secondary));
    h.advance(Duration::from_millis(300));
    pick(&mut h, "Move to Folder");
    pick(&mut h, "New Folder");
    assert_eq!(filed(&h), 1, "the row is in the folder:\n{}", h.html());
    assert!(
        h.html().contains("data-editing=\"true\""),
        "the name is asked for:\n{}",
        h.html()
    );
    type_text(&mut h, "Servers");
    h.send(Input::key(ShortcutKey::Enter));
    h.advance(Duration::from_millis(400));
    let html = h.html();
    assert!(!html.contains("data-editing=\"true\""), "named");
    assert!(
        html.contains("aria-label=\"Servers\""),
        "called Servers:\n{html}"
    );
    // A press closes it, and another opens it again.
    assert_eq!(
        expanded(&h).as_deref(),
        Some("true"),
        "a new folder is open"
    );
    let folder = rects(&h, ".temor-side-folder .ds-row[aria-expanded]")[0].centre();
    h.send(Input::click(folder));
    h.advance(Duration::from_millis(400));
    assert_eq!(expanded(&h).as_deref(), Some("false"), "closed");
    h.send(Input::click(folder));
    h.advance(Duration::from_millis(400));
    assert_eq!(expanded(&h).as_deref(), Some("true"), "open again");
    // A loose row dropped on the folder goes in.
    let loose: Vec<_> = {
        let filed_tops: Vec<f64> = rects(&h, ".temor-side-folder .temor-tab-box")
            .iter()
            .map(|row| row.y)
            .collect();
        rects(&h, ".temor-side .temor-tab-box")
            .into_iter()
            .filter(|row| !filed_tops.contains(&row.y))
            .collect()
    };
    assert_eq!(loose.len(), 2, "two loose rows");
    let folder = rects(&h, ".temor-side-folder .ds-row[aria-expanded]")[0].centre();
    h.send(Input::drag(loose[0].centre(), folder, 12));
    h.advance(Duration::from_millis(400));
    assert_eq!(filed(&h), 2, "dropped in:\n{}", h.html());
    // Delete Folder leaves its tabs.
    h.send(Input::press(folder, PointerButton::Secondary));
    h.advance(Duration::from_millis(300));
    pick(&mut h, "Delete Folder");
    assert!(
        rects(&h, ".temor-side-folder").is_empty(),
        "the folder went"
    );
    assert_eq!(drive::rows(&h), 3, "its tabs stayed");
}
