//! The text a Yes types into a running shell (PLAN P17), run by real shells with piped input and
//! a scratch HOME: the integration is loaded, the file it wrote is gone, and the lines are not
//! left in history. A missing shell is a printed skip, or a failure under TEMOR_REQUIRE_TOOLS=1.
//! nushell is not here: it reads piped input as one script, so a file written by an earlier
//! line cannot be sourced; that needs a terminal.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use temor_shell::integration_scripts;
use temor_ssh::{InnerLoadLine, InnerShell, LoadToken};

use crate::harness::{Scratch, contains, require, scratch, system_path};

static NEXT: AtomicU64 = AtomicU64::new(0);

/// A token no other test (or run) uses, so parallel tests keep their own files in /tmp.
fn token() -> LoadToken {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.subsec_nanos());
    LoadToken::from_words(
        (u64::from(std::process::id()) << 32) | u64::from(nanos),
        NEXT.fetch_add(1, Ordering::Relaxed),
    )
}

/// Runs `program args` with `input` on its stdin: stdout and stderr together, and stdout alone (an
/// interactive bash echoes the typed lines on stderr, so what history lists is on stdout).
fn run_shell(s: &Scratch, program: &Path, args: &[&str], input: &str) -> (Vec<u8>, Vec<u8>) {
    let mut child = Command::new(program)
        .args(args)
        .env_clear()
        .env("HOME", &s.home)
        .env("PATH", system_path())
        .env("TERM", "xterm-256color")
        .env("LANG", "C")
        .env("HISTFILE", s.home.join("history"))
        .env("HISTSIZE", "1000")
        .env("SAVEHIST", "1000")
        .current_dir(&s.work)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn shell");
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(input.as_bytes());
    }
    let output = child.wait_with_output().expect("wait");
    let mut text = output.stdout.clone();
    text.extend(output.stderr);
    (text, output.stdout)
}

/// Types the load text, then `after`, into `shell`: all it printed, its stdout, and the token.
fn load_into(
    shell: &str,
    typed_for: InnerShell,
    args: &[&str],
    after: &str,
) -> Option<(Vec<u8>, Vec<u8>, LoadToken)> {
    let program = require(shell)?;
    let s = scratch();
    let token = token();
    let line = InnerLoadLine::build_for(typed_for, &integration_scripts(), &token).expect("build");
    let (text, out) = run_shell(&s, &program, args, &format!("{}{after}", line.as_str()));
    Some((text, out, token))
}

#[test]
fn bash_loads_the_integration_and_forgets_the_lines() {
    let after = "echo LOADED=${_temor_integrated-no}\nhistory\nexit\n";
    let Some((text, out, token)) = load_into(
        "bash",
        InnerShell::Bash,
        &["--norc", "--noprofile", "-i"],
        after,
    ) else {
        return;
    };
    let shown = String::from_utf8_lossy(&text).into_owned();
    assert!(contains(&text, "LOADED=1"), "{shown}");
    assert!(contains(&text, "\x1b]133;A"), "no prompt mark: {shown}");
    assert!(
        contains(&out, "echo LOADED"),
        "history lists nothing: {shown}"
    );
    assert!(!contains(&out, &token.marker()), "left in history: {shown}");
    assert!(!contains(&out, "_temor_hs_off"), "left in history: {shown}");
    assert!(!Path::new(&token.path()).exists(), "the file stayed");
}

#[test]
fn zsh_loads_the_integration_and_keeps_the_lines_out_of_history() {
    let after = "print LOADED=${_temor_integrated-no}\nfc -l 1\nexit\n";
    let Some((text, _, token)) = load_into("zsh", InnerShell::Zsh, &["-f", "-i"], after) else {
        return;
    };
    let shown = String::from_utf8_lossy(&text).into_owned();
    assert!(contains(&text, "LOADED=1"), "{shown}");
    assert!(contains(&text, "\x1b]133;A"), "no prompt mark: {shown}");
    assert!(
        !contains(&text, &token.marker()),
        "left in history: {shown}"
    );
    assert!(!Path::new(&token.path()).exists(), "the file stayed");
}

#[test]
fn fish_loads_the_integration_in_place_and_stays_running() {
    let after = "echo LOADED=$_temor_integrated\nhistory\necho STILL_HERE\nexit\n";
    let Some((text, _, token)) = load_into("fish", InnerShell::Fish, &["--no-config", "-i"], after)
    else {
        return;
    };
    let shown = String::from_utf8_lossy(&text).into_owned();
    assert!(contains(&text, "LOADED=1"), "{shown}");
    assert!(
        contains(&text, "STILL_HERE"),
        "the load ended the shell: {shown}"
    );
    assert!(
        !contains(&text, &token.marker()),
        "left in history: {shown}"
    );
    assert!(!Path::new(&token.path()).exists(), "the file stayed");
}

#[test]
fn a_plain_shell_gets_an_empty_file_and_no_marks() {
    let after = "echo DONE\nexit\n";
    let Some((text, _, token)) = load_into("dash", InnerShell::Bash, &["-i"], after) else {
        return;
    };
    assert!(contains(&text, "DONE"));
    assert!(!contains(&text, "\x1b]133"));
    let path = token.path();
    let left = std::fs::metadata(&path).map(|meta| meta.len());
    let _ = std::fs::remove_file(&path);
    assert_eq!(
        left.ok(),
        Some(0),
        "an unknown shell is given nothing to source"
    );
}
