//! The window's title follows the pane (PLAN §7): the program's own title when it set one, else
//! the folder the shell is in. Read from the titles the harness's recorded window host kept.

use std::time::{Duration, Instant};

use ds::base::vocab::ShortcutKey;
use ds_harness::{Driver, Harness, Input, WindowHosting};
use temor_ui::view::probe::Probe;

use super::window::{harness_hosting, wait_until};

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

fn wait_for_title(h: &mut Harness, what: &str, ok: impl Fn(&str) -> bool) -> String {
    let start = Instant::now();
    loop {
        if let Some(title) = h.window_title().filter(|title| ok(title)) {
            return title;
        }
        h.advance(Duration::from_millis(25));
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "timed out waiting for {what}; titles so far: {:?}",
            h.window_titles()
        );
    }
}

#[test]
fn the_title_is_the_programs_then_the_folder() {
    if !super::support::tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness_hosting(
        "window-title",
        &probe,
        None,
        |_| {},
        WindowHosting::Recorded,
    );
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    h.send(Input::click(ds::prelude::Point {
        x: ds::prelude::Px(300.0),
        y: ds::prelude::Px(200.0),
    }));
    h.advance(Duration::from_millis(100));

    // The program sets a title (OSC 2): the window shows it.
    type_line(&mut h, "printf '\\033]2;scenario title\\007'");
    wait_for_title(&mut h, "the program's title", |t| t == "scenario title");

    // It clears the title: the window falls back to the folder the shell reported (the scratch
    // home the window starts in).
    type_line(&mut h, "printf '\\033]2;\\007'");
    let folder = wait_for_title(&mut h, "the folder", |t| t != "scenario title");
    assert!(
        folder.ends_with("/home"),
        "the folder the shell is in, not {folder:?}"
    );
}
