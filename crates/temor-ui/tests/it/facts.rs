//! The context chips in the window (PLAN P16), in ds-harness: a scratch repository with one
//! changed file is the shell's folder, and the chips over the prompt's header line (and the tab's
//! row) say so.
//! The DOM is read back; no pixels, no OS input, no real HOME.

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use ds_harness::Harness;
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

/// Which prompt the shell draws: the plain `$ ` the other tests read, or temor's own.
#[derive(Clone, Copy, PartialEq)]
enum Prompt {
    Plain,
    Temor,
}

/// A window whose shell starts in a scratch repository with one changed file, at its prompt.
fn changed_repository(name: &str, probe: &Probe, prompt: Prompt) -> Harness {
    let mut h = harness_with(name, probe, None, |ambient| {
        if prompt == Prompt::Temor {
            ambient.env.set("TEMOR_PROMPT", "temor");
        }
        let home = &ambient.home;
        git(home, &["init", "-q", "-b", "main"]);
        std::fs::write(home.join("a.txt"), "one\n").expect("write");
        git(home, &["add", "a.txt"]);
        git(home, &["commit", "-q", "-m", "first"]);
        std::fs::write(home.join("a.txt"), "one\ntwo\n").expect("write");
    });
    let at_prompt = move |s: &str| match prompt {
        Prompt::Plain => s.contains('$'),
        // temor's header line: the home folder and the branch.
        Prompt::Temor => s.contains("~  main"),
    };
    wait_until(&mut h, probe, "the prompt", at_prompt);
    h
}

/// The chips row once it says the change, and the whole page then.
fn chips_with_the_change(h: &mut Harness) -> (String, String) {
    let start = Instant::now();
    loop {
        h.advance(Duration::from_millis(50));
        let html = h.html();
        if let Some(chips) = chips_of(&html).filter(|c| c.contains("+1")) {
            return (chips, html);
        }
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "no chips with the change:\n{html}"
        );
    }
}

#[test]
fn a_changed_repository_shows_its_branch_and_changes_in_the_chips_and_on_the_tab() {
    if !tools_present() || !git_present() {
        return;
    }
    let probe = Probe::new();
    // The shell starts in the home folder, so the repository is made there, after the harness has
    // made the folder clean and before the shell is started.
    let mut h = changed_repository("facts-chips", &probe, Prompt::Plain);
    let (chips, html) = chips_with_the_change(&mut h);
    // The tab's row says the branch.
    assert!(
        html.contains("main"),
        "the tab row lacks the branch:\n{html}"
    );
    for want in ["+1", "\u{2212}0", "1 file"] {
        assert!(chips.contains(want), "no {want:?} in the chips:\n{chips}");
    }
    // A prompt of the shell's own keeps its text: the chips end at the right, without the folder
    // and the branch.
    assert!(
        html.contains("data-place=\"end\""),
        "the chips are not at the end:\n{html}"
    );
    for kind in ["folder", "branch"] {
        assert!(
            !chips.contains(&format!("data-kind=\"{kind}\"")),
            "a {kind} chip beside the shell's own text:\n{chips}"
        );
    }
}

#[test]
fn over_temors_prompt_the_chips_say_the_folder_and_branch_first() {
    if !tools_present() || !git_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = changed_repository("facts-chips-over", &probe, Prompt::Temor);
    let (chips, html) = chips_with_the_change(&mut h);
    assert!(
        html.contains("data-place=\"over\""),
        "the chips are not over the header line:\n{html}"
    );
    let at = |kind: &str| {
        chips
            .find(&format!("data-kind=\"{kind}\""))
            .unwrap_or_else(|| panic!("no {kind} chip:\n{chips}"))
    };
    assert!(
        at("folder") < at("branch") && at("branch") < at("changes"),
        "the folder, then the branch, then the changes:\n{chips}"
    );
    for want in [">~<", ">main<", "+1"] {
        assert!(chips.contains(want), "no {want:?} in the chips:\n{chips}");
    }
}

/// An older block's header line carries the chips its prompt had: the folder, the branch and the
/// change, over the shell's own text, as the prompt's do.
#[test]
fn an_older_blocks_header_keeps_its_prompts_chips() {
    if !tools_present() || !git_present() {
        return;
    }
    use ds::base::vocab::ShortcutKey;
    use ds_harness::Input;
    let probe = Probe::new();
    let mut h = changed_repository("facts-chips-older", &probe, Prompt::Temor);
    let _ = chips_with_the_change(&mut h);
    h.send(Input::click(ds::prelude::Point {
        x: ds::prelude::Px(700.0),
        y: ds::prelude::Px(300.0),
    }));
    for c in "echo older-done".chars() {
        h.send(Input::key(match c {
            ' ' => ShortcutKey::Space,
            c => ShortcutKey::Char(c),
        }));
    }
    h.send(Input::key(ShortcutKey::Enter));
    let start = Instant::now();
    let header = loop {
        h.advance(Duration::from_millis(50));
        let html = h.html();
        let found = html
            .split("data-place=\"header\"")
            .nth(1)
            .map(|after| after[..after.find("</div>").unwrap_or(after.len())].to_owned());
        if let Some(found) = found {
            break found;
        }
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "no chips over the older block's header:\n{}",
            probe.screen()
        );
    };
    for want in [">~<", ">main<", "+1"] {
        assert!(
            header.contains(want),
            "no {want:?} over the older header:\n{header}"
        );
    }
}
