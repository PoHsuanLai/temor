//! The ⌘T bar and the quick terminal in the window (ds-harness, event dispatch only).
//!
//! The ⌘ chords arrive in the Ctrl+Shift form keycap sends (the harness's keymap is our
//! desktop's); the quick terminal's is ⇧⌘Y as Super+Shift+Y.

use std::time::Duration;

use ds::base::vocab::ShortcutKey;

use ds::window::vocab::PageZoom;
use ds_harness::{FocusState, Harness, Input, WindowHosting};
use temor_ui::view::probe::Probe;

use super::drive::{chord, focus_pane, panes, rows, started, type_line, type_text, wait_until};
use super::window::harness_hosting;

fn bar_open(h: &Harness) -> bool {
    // The element, not the stylesheet that names its class.
    h.html().contains("class=\"ds-palette\"")
}

/// ⌘T opens the bar; typing a command that is on the path leads with its Run row; Enter runs it
/// in a new tab; Escape closes the bar and the keys are the shell's again.
#[test]
fn the_bar_opens_ranks_runs_and_hands_the_keys_back() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("bar-flow", &probe);
    focus_pane(&mut h);
    assert!(!bar_open(&h));

    chord(&mut h, 't');
    assert!(bar_open(&h), "⌘T opens the bar:\n{}", h.html());
    assert!(
        h.html().contains("New tab"),
        "an empty bar leads with a plain tab"
    );

    type_text(&mut h, "echo bar-ok");
    h.advance(Duration::from_millis(300));
    assert!(
        h.text_of(".ds-palette")
            .unwrap_or_default()
            .contains("Run `echo bar-ok` in a new tab"),
        "echo is on the path, so the Run row leads:\n{:?}",
        h.text_of(".ds-palette")
    );
    h.send(Input::key(ShortcutKey::Enter));
    h.advance(Duration::from_millis(300));
    assert!(!bar_open(&h), "a pick closes the bar");
    assert_eq!(rows(&h), 2, "the command opened a tab");
    wait_until(&mut h, &probe, "the command's output", |s| {
        s.lines().any(|l| l.trim_end() == "bar-ok")
    });

    chord(&mut h, 't');
    assert!(bar_open(&h));
    h.send(Input::key(ShortcutKey::Escape));
    h.advance(Duration::from_millis(300));
    assert!(!bar_open(&h), "Escape closes it");
    assert_eq!(rows(&h), 2, "and opens nothing");
    type_line(&mut h, "echo after-bar");
    wait_until(&mut h, &probe, "typing after the bar", |s| {
        s.lines().any(|l| l.trim_end() == "after-bar")
    });
}

/// ⌘F puts the keyboard in the find field: typing lands there and the shell sees nothing; Escape
/// closes the bar and the keys are the shell's again.
#[test]
fn the_find_field_takes_the_keyboard_and_gives_it_back() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("find-keyboard", &probe);
    focus_pane(&mut h);
    assert_eq!(h.count(".temor-find"), 0);

    chord(&mut h, 'f');
    assert_eq!(h.count(".temor-find"), 1, "⌘F opens the find bar");
    assert_eq!(
        h.focus_of(".temor-find input"),
        FocusState::Focused,
        "the field has the caret"
    );
    for c in "zq".chars() {
        h.send(Input::key(ShortcutKey::Char(c)));
    }
    h.advance(Duration::from_millis(300));
    assert_eq!(
        h.attr(".temor-find input", "value").as_deref(),
        Some("zq"),
        "the field holds what was typed"
    );
    assert!(
        !probe.screen().contains("zq"),
        "the shell heard nothing:\n{}",
        probe.screen()
    );

    h.send(Input::key(ShortcutKey::Escape));
    h.advance(Duration::from_millis(300));
    assert_eq!(h.count(".temor-find"), 0, "Escape closes it");
    type_line(&mut h, "echo find-back");
    wait_until(&mut h, &probe, "typing after the find", |s| {
        s.lines().any(|l| l.trim_end() == "find-back")
    });
}

/// Every declared action is a row under a header: "spl" lists Split right and Split down under
/// Actions, with no per-row kind caption, and picking one does what its chord does.
#[test]
fn the_bar_lists_window_actions_under_headers_and_runs_them() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("bar-actions", &probe);
    focus_pane(&mut h);

    chord(&mut h, 't');
    let empty = h.text_of(".ds-palette").unwrap_or_default();
    assert!(
        empty.contains("Actions"),
        "a header over the actions:\n{empty}"
    );
    for c in "spl".chars() {
        h.send(Input::key(ShortcutKey::Char(c)));
    }
    h.advance(Duration::from_millis(300));
    let listed = h.text_of(".ds-palette").unwrap_or_default();
    assert!(
        listed.contains("Split right") && listed.contains("Split down"),
        "both splits are rows:\n{listed}"
    );
    assert!(listed.contains("Actions"), "under their header:\n{listed}");
    assert!(
        !listed.contains("Recent command"),
        "no per-row kind caption:\n{listed}"
    );

    h.send(Input::key(ShortcutKey::Enter));
    h.advance(Duration::from_millis(400));
    assert!(!bar_open(&h), "a pick closes the bar");
    assert_eq!(panes(&h), 2, "Split right split the pane");
}

/// "Detach running program" works only in a pane the background service keeps, with a program
/// running: an ordinary pane's bar does not list it.
#[test]
fn the_bar_lists_detach_only_where_it_works() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("bar-detach", &probe);
    focus_pane(&mut h);
    chord(&mut h, 't');
    type_text(&mut h, "detach");
    h.advance(Duration::from_millis(300));
    let rows = h.text_of(".ds-palette").unwrap_or_default();
    assert!(!rows.contains("Detach running program"), "{rows}");
}

/// The bar pins the tab in front and lets it go again, and opens Downloads: what the sidebar
/// shows without a button of its own is still a few keys away.
#[test]
fn the_bar_pins_the_tab_and_opens_downloads() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("bar-pin", &probe);
    focus_pane(&mut h);
    let tiles = |h: &Harness| h.html().matches("class=\"temor-pin\"").count();
    let listed = |h: &mut Harness, typed: &str| {
        chord(h, 't');
        type_text(h, typed);
        h.advance(Duration::from_millis(300));
        h.text_of(".ds-palette").unwrap_or_default()
    };

    let rows = listed(&mut h, "pin tab");
    assert!(
        rows.contains("Pin tab") && !rows.contains("Unpin tab"),
        "{rows}"
    );
    h.send(Input::key(ShortcutKey::Enter));
    h.advance(Duration::from_millis(400));
    assert_eq!(tiles(&h), 1, "pinned:\n{}", h.html());

    let rows = listed(&mut h, "unpin");
    assert!(rows.contains("Unpin tab"), "{rows}");
    h.send(Input::key(ShortcutKey::Enter));
    h.advance(Duration::from_millis(400));
    assert_eq!(tiles(&h), 0, "let go:\n{}", h.html());

    let rows = listed(&mut h, "downloads");
    assert!(rows.contains("Downloads"), "{rows}");
    h.send(Input::key(ShortcutKey::Enter));
    h.advance(Duration::from_millis(400));
    assert!(
        h.html().contains("class=\"temor-dl-list\""),
        "the Downloads list is open:\n{}",
        h.html()
    );
}

/// The quick terminal hides without ending its shell: the variable set before the hide is still
/// there after the show, which a new shell would not have.
#[test]
fn the_quick_terminal_keeps_its_shell_when_hidden() {
    if !super::support::window_tools_present() {
        return;
    }
    let quick = |h: &mut Harness| {
        h.send(Input::chord(
            &[ShortcutKey::Super, ShortcutKey::Shift],
            ShortcutKey::Char('y'),
        ));
        h.advance(Duration::from_millis(300));
    };
    let probe = Probe::new();
    let mut h = started("bar-quick", &probe);
    focus_pane(&mut h);
    assert_eq!(panes(&h), 1);

    quick(&mut h);
    assert_eq!(
        panes(&h),
        2,
        "the quick terminal is over the tab:\n{}",
        h.html()
    );
    wait_until(&mut h, &probe, "the quick shell's prompt", |s| {
        s.contains('$')
    });
    type_line(&mut h, "export QV=kept");
    h.advance(Duration::from_millis(300));

    quick(&mut h);
    assert_eq!(panes(&h), 1, "hidden");
    assert_eq!(rows(&h), 1, "and it never became a tab");

    quick(&mut h);
    assert_eq!(panes(&h), 2, "shown again");
    type_line(&mut h, "printenv QV");
    wait_until(&mut h, &probe, "the same shell's variable", |s| {
        s.lines().any(|l| l.trim_end() == "kept")
    });
}

/// ⌘=, ⌘- and ⌘0 zoom the window in, out and back (the standard Bigger, Smaller and Actual
/// Size): the window host is asked, and the shell gets none of them, so `echo a`, the three
/// keys, then `b` prints `ab`.
#[test]
fn the_zoom_keys_zoom_the_window_and_type_nothing() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness_hosting("zoom-keys", &probe, None, |_| {}, WindowHosting::Recorded);
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    focus_pane(&mut h);
    type_text(&mut h, "echo a");
    for key in ['=', '-', '0'] {
        h.send(Input::chord(&[ShortcutKey::Super], ShortcutKey::Char(key)));
    }
    type_line(&mut h, "b");
    wait_until(&mut h, &probe, "ab on its own line", |s| {
        s.lines().any(|line| line.trim_end() == "ab")
    });
    assert_eq!(
        h.page_zooms(),
        [PageZoom::Bigger, PageZoom::Smaller, PageZoom::ActualSize]
    );
}

/// The bar runs the zoom too: "zoom in" leads with Zoom in, and Enter asks the window.
#[test]
fn the_bar_zooms_the_window() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness_hosting("bar-zoom", &probe, None, |_| {}, WindowHosting::Recorded);
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    focus_pane(&mut h);
    chord(&mut h, 't');
    type_text(&mut h, "zoom in");
    h.advance(Duration::from_millis(300));
    let rows = h.text_of(".ds-palette").unwrap_or_default();
    assert!(rows.contains("Zoom in"), "{rows}");
    h.send(Input::key(ShortcutKey::Enter));
    h.advance(Duration::from_millis(400));
    assert_eq!(h.page_zooms(), [PageZoom::Bigger]);
}
