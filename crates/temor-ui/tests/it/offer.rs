//! The shell offer in the window (ds-harness, event dispatch only). A stand-in `ssh` on a
//! private `PATH` keeps running without printing, which is all the timing rule looks at. It
//! saves the first bytes it is sent, so the test sees what a Yes writes to the pane, and
//! nothing else leaves the scratch folders.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ds::base::vocab::ShortcutKey;
use ds::prelude::{Point, Px};
use ds_harness::{Driver, Harness, Input};
use temor_ui::view::probe::Probe;

use super::support::{scratch_path, tools_present};
use super::window::{harness_with, wait_until};

const QUESTION: &str = "Show commands as blocks in this shell?";

/// A window whose `ssh` reads 40 bytes of raw input into `saved`, then sleeps. The first
/// command, `ssh box`, has been typed.
fn window_running_ssh(name: &str, saved: &Path) -> (Harness, Probe) {
    let bin = std::env::temp_dir().join(format!("tmr-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&bin);
    std::fs::create_dir_all(&bin).expect("bin folder");
    let script = format!(
        "#!/bin/sh\nstty -icanon -echo\nhead -c 40 > '{}'\nexec sleep 20\n",
        saved.display()
    );
    let ssh = bin.join("ssh");
    std::fs::write(&ssh, script).expect("stand-in ssh");
    std::fs::set_permissions(&ssh, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let probe = Probe::new();
    let path = format!("{}:/usr/bin:/bin", bin.display());
    let mut h = harness_with(name, &probe, None, move |ambient| {
        ambient.env.set("PATH", path);
    });
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    h.send(Input::click(Point {
        x: Px(300.0),
        y: Px(200.0),
    }));
    h.advance(Duration::from_millis(100));
    for c in "ssh box".chars() {
        let key = match c {
            ' ' => ShortcutKey::Space,
            c => ShortcutKey::Char(c),
        };
        h.send(Input::key(key));
    }
    h.send(Input::key(ShortcutKey::Enter));
    (h, probe)
}

fn wait_html(h: &mut Harness, what: &str, ok: impl Fn(&str) -> bool) {
    let start = Instant::now();
    while !ok(&h.html()) {
        h.advance(Duration::from_millis(25));
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "timed out waiting for {what}:\n{}",
            h.html()
        );
    }
}

fn press(h: &mut Harness, selector: &str) {
    let at = h
        .centre(selector)
        .unwrap_or_else(|| panic!("no {selector}:\n{}", h.html()));
    h.send(Input::click(at));
    h.advance(Duration::from_millis(100));
}

fn saved_path(name: &str) -> PathBuf {
    scratch_path(name).join("saved-input")
}

#[test]
fn yes_types_the_setup_lines_into_the_pane_and_nothing_before() {
    if !tools_present() {
        return;
    }
    let name = "offer-yes";
    let (mut h, _probe) = window_running_ssh(name, &saved_path(name));
    wait_html(&mut h, "the offer", |html| html.contains(QUESTION));
    let before = std::fs::read(saved_path(name)).unwrap_or_default();
    assert!(before.is_empty(), "nothing is typed before Yes");

    press(&mut h, ".ds-inline-banner .ds-button:first-of-type");
    let start = Instant::now();
    let typed = loop {
        h.advance(Duration::from_millis(25));
        let bytes = std::fs::read(saved_path(name)).unwrap_or_default();
        if bytes.len() >= 40 {
            break bytes;
        }
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "nothing was typed"
        );
    };
    assert!(
        typed.starts_with(b" [ -n \"$ZSH_VERSION\" ]"),
        "{}",
        String::from_utf8_lossy(&typed)
    );
    assert!(!h.html().contains(QUESTION), "the offer goes away");
}

#[test]
fn never_for_the_program_is_kept_in_the_settings_file() {
    if !tools_present() {
        return;
    }
    let name = "offer-never";
    let (mut h, _probe) = window_running_ssh(name, &saved_path(name));
    wait_html(&mut h, "the offer", |html| html.contains(QUESTION));
    assert!(h.html().contains("Never for ssh"));

    press(&mut h, ".ds-inline-banner .ds-button:last-of-type");
    assert!(!h.html().contains(QUESTION));
    let settings = scratch_path(name).join("home/config/terminal.conf");
    let text = std::fs::read_to_string(&settings).expect("the choice is written");
    assert_eq!(text, "offer.remove = ssh\n");
}
