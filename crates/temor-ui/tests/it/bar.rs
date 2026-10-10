//! The ⌘T bar and the quick terminal in the window (ds-harness, event dispatch only).
//!
//! The ⌘ chords arrive in the Ctrl+Shift form keycap sends (the harness's keymap is our
//! desktop's); the quick terminal's is ⇧⌘Y as Super+Shift+Y.

use std::time::Duration;

use ds::base::vocab::ShortcutKey;
use ds::prelude::{Point, Px};
use ds_harness::{Driver, Harness, Input, Query};
use temor_ui::view::probe::Probe;

use super::window::{chord, harness, panes, rows, wait_until};

fn type_line(h: &mut Harness, text: &str) {
    for c in text.chars() {
        let key = match c {
            ' ' => ShortcutKey::Space,
            c => ShortcutKey::Char(c),
        };
        h.send(Input::key(key));
    }
    h.send(Input::key(ShortcutKey::Enter));
}

fn click_pane(h: &mut Harness) {
    h.send(Input::click(Point {
        x: Px(300.0),
        y: Px(200.0),
    }));
    h.advance(Duration::from_millis(200));
}

fn bar_open(h: &Harness) -> bool {
    // The element, not the stylesheet that names its class.
    h.html().contains("class=\"ds-palette\"")
}

/// ⌘T opens the bar; typing a command that is on the path leads with its Run row; Enter runs it
/// in a new tab; Escape closes the bar and the keys are the shell's again.
#[test]
fn the_bar_opens_ranks_runs_and_hands_the_keys_back() {
    if !super::support::tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness("bar-flow", &probe);
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    click_pane(&mut h);
    assert!(!bar_open(&h));

    chord(&mut h, 't');
    assert!(bar_open(&h), "⌘T opens the bar:\n{}", h.html());
    assert!(
        h.html().contains("New tab"),
        "an empty bar leads with a plain tab"
    );

    for c in "echo bar-ok".chars() {
        let key = match c {
            ' ' => ShortcutKey::Space,
            c => ShortcutKey::Char(c),
        };
        h.send(Input::key(key));
    }
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

/// The quick terminal hides without ending its shell: the variable set before the hide is still
/// there after the show, which a new shell would not have.
#[test]
fn the_quick_terminal_keeps_its_shell_when_hidden() {
    if !super::support::tools_present() {
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
    let mut h = harness("bar-quick", &probe);
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    click_pane(&mut h);
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
