//! The context chips in the window (PLAN P16), in ds-harness: a scratch repository with one
//! changed file is the shell's folder, and the chips over the prompt (and the tab's row) say so.
//! The DOM is read back; no pixels, no OS input, no real HOME.

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use ds_harness::{Driver, Harness};
use temor_ui::view::probe::Probe;

use super::support::tools_present;
use super::window::{harness_with, wait_until};

/// Runs the real git in `dir` with a fixed identity and no global configuration.
fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn git_present() -> bool {
    let found = temor_pty::find_program("git", std::ffi::OsStr::new("/usr/bin:/bin")).is_some();
    if !found && std::env::var("TEMOR_REQUIRE_TOOLS").is_ok_and(|v| v == "1") {
        panic!("git is required (TEMOR_REQUIRE_TOOLS=1)");
    }
    found
}

/// The markup of the chips row, or none while it is not drawn.
fn chips_of(html: &str) -> Option<String> {
    let after = html.split("class=\"temor-fact-chips").nth(1)?;
    let end = after.find("</div>").unwrap_or(after.len());
    Some(after[..end].to_owned())
}

/// A window whose shell starts in a scratch repository with one changed file, at its prompt.
fn changed_repository(name: &str, probe: &Probe) -> Harness {
    let mut h = harness_with(name, probe, None, |ambient| {
        let home = &ambient.home;
        git(home, &["init", "-q", "-b", "main"]);
        std::fs::write(home.join("a.txt"), "one\n").expect("write");
        git(home, &["add", "a.txt"]);
        git(home, &["commit", "-q", "-m", "first"]);
        std::fs::write(home.join("a.txt"), "one\ntwo\n").expect("write");
    });
    wait_until(&mut h, probe, "the prompt", |s| s.contains('$'));
    h
}

#[test]
fn a_changed_repository_shows_its_branch_and_changes_in_the_chips_and_on_the_tab() {
    if !tools_present() || !git_present() {
        return;
    }
    let probe = Probe::new();
    // The shell starts in the home folder, so the repository is made there, after the harness has
    // made the folder clean and before the shell is started.
    let mut h = changed_repository("facts-chips", &probe);
    let start = Instant::now();
    let chips = loop {
        h.advance(Duration::from_millis(50));
        let html = h.html();
        if let Some(chips) = chips_of(&html).filter(|c| c.contains("+1")) {
            // The tab's row says the same, shorter.
            assert!(
                html.matches("main \u{2022}").count() >= 2,
                "the tab row lacks the branch and dot:\n{html}"
            );
            break chips;
        }
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "no chips with the change:\n{html}"
        );
    };
    for want in ["main \u{2022}", "+1 \u{2212}0", "1 file"] {
        assert!(chips.contains(want), "no {want:?} in the chips:\n{chips}");
    }
}
