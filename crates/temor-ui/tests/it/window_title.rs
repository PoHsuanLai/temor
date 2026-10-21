//! The window's title follows the pane (PLAN §7): the program's own title when it set one, else
//! the folder the shell is in. Read from the titles the harness's recorded window host kept.

use std::time::{Duration, Instant};

use ds_harness::{Harness, Input, WindowHosting};
use temor_ui::view::probe::Probe;

use super::drive::{type_line, wait_until};
use super::window::harness_hosting;

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
    if !super::support::window_tools_present() {
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
    // Each title is held by a sleep: the shell's prompt may set a title of its own (Fedora's
    // /etc/bashrc does), which would replace it as soon as the command returns.
    type_line(&mut h, "printf '\\033]2;scenario title\\007'; sleep 3");
    wait_for_title(&mut h, "the program's title", |t| t == "scenario title");

    // It clears the title: the window falls back to the folder the shell reported (the scratch
    // home the window starts in). The prompt between the two commands may set its own title
    // first, so the wait is for the folder itself.
    type_line(&mut h, "printf '\\033]2;\\007'; sleep 3");
    wait_for_title(&mut h, "the folder the shell is in", |t| {
        t.ends_with("/home")
    });
}
