//! The OSC 133 and OSC 7 marks each shell's integration emits around `true` and `false`.

use std::path::PathBuf;

use temor_pty::prepare_shell;
use temor_shell::{ChildEnv, Shell, ShellStart};

use crate::support::{Scratch, Term, find, program_or_skip, setsid, size, url_path};

// fish 4 adds options (`A;click_events=1`, `C;cmdline_url=…`), so A and C match as prefixes.
const PROMPT_START: &str = "\x1b]133;A";
// The marks end with ST or BEL as the shell chooses (fish 4 changed which), so only the
// mark itself is matched.
const PROMPT_END: &str = "\x1b]133;B";
const OUTPUT_START: &str = "\x1b]133;C";

/// Runs `true` then `false` in `shell` and checks C, then D with the status, then the next
/// prompt's OSC 7 and A, then B.
fn marks_around_true_and_false(shell: PathBuf, prepare: impl Fn(&Scratch)) {
    let scratch = Scratch::new();
    prepare(&scratch);
    let spec = prepare_shell(
        &Shell::at(shell),
        ShellStart::Interactive,
        scratch.contract_env(&ChildEnv::new()),
        &scratch.runtime,
        scratch.cwd.clone(),
        size(),
    )
    .expect("prepare shell");
    let mut term = Term::spawn(&spec, &setsid());
    assert!(
        term.read_until_has(0, PROMPT_END),
        "no first prompt: {:?}",
        term.text()
    );
    let first = term.out.clone();
    assert!(
        find(&first, b"\x1b]133;D").is_none(),
        "D before any command"
    );
    let cwd = scratch.cwd.canonicalize().expect("cwd");
    let osc7 = "\x1b]7;file://";
    let at = find(&first, osc7.as_bytes()).expect("OSC 7 at the first prompt");
    // Ended by ST, or by BEL as fish's own OSC 7 does.
    let rest = at + osc7.len();
    let url_end = first[rest..]
        .iter()
        .position(|&b| b == 0x1b || b == 0x07)
        .expect("OSC 7 end")
        + rest;
    let url = String::from_utf8_lossy(&first[rest..url_end]).into_owned();
    assert!(
        url.ends_with(&url_path(&cwd)),
        "OSC 7 {url:?} is not {cwd:?}"
    );
    assert!(url.contains("%20") && url.contains("%C3%A9"), "{url:?}");

    for (command, status) in [("true", 0), ("false", 1)] {
        let start = term.out.len();
        term.send(&format!("{command}\r"));
        // The status, then the mark's end: ST or BEL as the shell chooses.
        let ends = [
            format!("\x1b]133;D;{status}\x1b\\"),
            format!("\x1b]133;D;{status}\x07"),
        ];
        let done = |out: &[u8]| ends.iter().find_map(|end| find(out, end.as_bytes()));
        assert!(
            term.read_until(|out| done(&out[start..]).is_some())
                && term.read_until_has(start, PROMPT_END),
            "{command}: no D;{status} and prompt: {:?}",
            term.text()
        );
        let out = &term.out[start..];
        let c = find(out, OUTPUT_START.as_bytes()).expect("C");
        let d = done(out).expect("D");
        let a = find(&out[d..], PROMPT_START.as_bytes()).expect("A after D") + d;
        let seven = find(&out[d..], osc7.as_bytes()).expect("OSC 7 after D") + d;
        let b = find(&out[a..], PROMPT_END.as_bytes()).expect("B after A") + a;
        assert!(
            c < d && d < a && seven < b && a < b,
            "{command}: order {out:?}"
        );
    }
    let text = term.text();
    for complaint in ["syntax error", "unbound variable", "command not found"] {
        assert!(!text.contains(complaint), "the shell complained: {text:?}");
    }
}

#[test]
fn bash_marks_true_and_false() {
    if crate::support::pty_unsupported("bash_marks_true_and_false") {
        return;
    }
    let Some(bash) = program_or_skip("bash", "bash_marks_true_and_false") else {
        return;
    };
    marks_around_true_and_false(bash, |_| {});
}

#[test]
fn zsh_marks_true_and_false() {
    if crate::support::pty_unsupported("zsh_marks_true_and_false") {
        return;
    }
    let Some(zsh) = program_or_skip("zsh", "zsh_marks_true_and_false") else {
        return;
    };
    // An empty .zshrc keeps zsh's first-run wizard away.
    marks_around_true_and_false(zsh, |scratch| {
        std::fs::write(scratch.home.join(".zshrc"), "PS1='%# '\n").expect(".zshrc");
    });
}

#[test]
fn fish_marks_true_and_false() {
    if crate::support::pty_unsupported("fish_marks_true_and_false") {
        return;
    }
    let Some(fish) = program_or_skip("fish", "fish_marks_true_and_false") else {
        return;
    };
    marks_around_true_and_false(fish, |_| {});
}

/// Why: joining with "; " turned `history -a;` into `history -a;; _temor_prompt`, a syntax
/// error at every prompt. Also runs the integration under `set -u`.
#[test]
fn bash_keeps_a_prompt_command_ending_in_a_semicolon() {
    let test = "bash_keeps_a_prompt_command_ending_in_a_semicolon";
    if crate::support::pty_unsupported(test) {
        return;
    }
    let Some(bash) = program_or_skip("bash", test) else {
        return;
    };
    marks_around_true_and_false(bash, |scratch| {
        std::fs::write(
            scratch.home.join(".bashrc"),
            "PROMPT_COMMAND=\"history -a;\"\nset -u\n",
        )
        .expect(".bashrc");
    });
}

/// Why: themes rebuild PS1 in their own precmd hook, after ours, and dropped the B mark.
#[test]
fn zsh_keeps_the_mark_under_a_theme_that_rebuilds_ps1() {
    let test = "zsh_keeps_the_mark_under_a_theme_that_rebuilds_ps1";
    if crate::support::pty_unsupported(test) {
        return;
    }
    let Some(zsh) = program_or_skip("zsh", test) else {
        return;
    };
    marks_around_true_and_false(zsh, |scratch| {
        std::fs::write(
            scratch.home.join(".zshrc"),
            "autoload -Uz add-zsh-hook\n_theme() { PS1='theme%# ' }\nadd-zsh-hook precmd _theme\n",
        )
        .expect(".zshrc");
    });
}

/// Why: the offer's probe line (which asks a running shell its name) stayed in zsh's history.
/// The hook keeps that one line out, nothing else, and nothing is rewritten.
#[test]
fn zsh_keeps_the_probe_line_out_of_history_and_everything_else_in() {
    let test = "zsh_keeps_the_probe_line_out_of_history_and_everything_else_in";
    if crate::support::pty_unsupported(test) {
        return;
    }
    let Some(zsh) = program_or_skip("zsh", test) else {
        return;
    };
    let scratch = Scratch::new();
    std::fs::write(scratch.home.join(".zshrc"), "PS1='%# '\n").expect(".zshrc");
    let spec = prepare_shell(
        &Shell::at(zsh),
        ShellStart::Interactive,
        scratch.contract_env(&ChildEnv::new()),
        &scratch.runtime,
        scratch.cwd.clone(),
        size(),
    )
    .expect("prepare shell");
    let mut term = Term::spawn(&spec, &setsid());
    assert!(term.read_until_has(0, PROMPT_END), "{:?}", term.text());

    let probe = " sh -c 'echo temor-shell:$(cat /proc/$PPID/comm)' sh\r";
    let start = term.out.len();
    term.send(probe);
    assert!(
        term.read_until_has(start, "temor-shell:zsh"),
        "{:?}",
        term.text()
    );
    // The bracket keeps this line from matching its own count.
    let start = term.out.len();
    term.send("print KEPT=$(fc -l 1 | grep -c 'print KEPT')\r");
    assert!(term.read_until_has(start, "KEPT=1"), "{:?}", term.text());
    let start = term.out.len();
    term.send("print PROBES=$(fc -l 1 | grep -c 'temor-shel[l]:')\r");
    assert!(term.read_until_has(start, "PROBES=0"), "{:?}", term.text());
}

/// An older shell installed under another name (`fish3`, `bash32`), linked as the name temor
/// recognises. The directory goes with the returned guard. These are extras a machine may have, so
/// a missing one skips the test even where `TEMOR_REQUIRE_TOOLS` is set.
fn renamed(program: &str, as_name: &str, test: &str) -> Option<(tempfile::TempDir, PathBuf)> {
    let Some(found) = crate::support::program(program) else {
        println!("note: {test} skipped: {program} is not installed");
        return None;
    };
    let dir = tempfile::tempdir().expect("link dir");
    let link = dir.path().join(as_name);
    std::os::unix::fs::symlink(found, &link).expect("link");
    Some((dir, link))
}

/// fish before 4 does not emit the marks itself; temor's fish integration does.
#[test]
fn fish_3_marks_true_and_false() {
    let test = "fish_3_marks_true_and_false";
    if crate::support::pty_unsupported(test) {
        return;
    }
    let Some((_dir, fish)) = renamed("fish3", "fish", test) else {
        return;
    };
    marks_around_true_and_false(fish, |_| {});
}

/// bash before 4.4 has no PS0: the output mark comes from a DEBUG trap, as Warp's bash-preexec
/// does (macOS's /bin/bash is 3.2), so its commands are marked as newer bash's are.
#[test]
fn bash_3_2_marks_true_and_false() {
    let test = "bash_3_2_marks_true_and_false";
    if crate::support::pty_unsupported(test) {
        return;
    }
    let Some((_dir, bash)) = renamed("bash32", "bash", test) else {
        return;
    };
    marks_around_true_and_false(bash, |_| {});
}

/// A DEBUG trap the person set in their startup files keeps running beside temor's on bash 3.2,
/// once per command, and an empty line marks nothing.
#[test]
fn bash_3_2_keeps_the_persons_own_debug_trap() {
    let test = "bash_3_2_keeps_the_persons_own_debug_trap";
    if crate::support::pty_unsupported(test) {
        return;
    }
    let Some((_dir, bash)) = renamed("bash32", "bash", test) else {
        return;
    };
    let scratch = Scratch::new();
    std::fs::write(
        scratch.home.join(".bashrc"),
        "_mine=0\ntrap '_mine=$((_mine + 1))' DEBUG\n",
    )
    .expect(".bashrc");
    let spec = prepare_shell(
        &Shell::at(bash),
        ShellStart::Interactive,
        scratch.contract_env(&ChildEnv::new()),
        &scratch.runtime,
        scratch.cwd.clone(),
        size(),
    )
    .expect("prepare shell");
    let mut term = Term::spawn(&spec, &setsid());
    assert!(term.read_until_has(0, PROMPT_END), "{:?}", term.text());
    let start = term.out.len();
    term.send("\r");
    assert!(term.read_until_has(start, PROMPT_END), "{:?}", term.text());
    assert!(
        find(&term.out[start..], OUTPUT_START.as_bytes()).is_none(),
        "an empty line was marked"
    );
    let start = term.out.len();
    term.send("_before=$_mine; true; echo MINE=$((_mine - _before))\r");
    let done = "\x1b]133;D;0";
    assert!(
        term.read_until_has(start, done) && term.read_until_has(start, PROMPT_END),
        "{:?}",
        term.text()
    );
    let out = String::from_utf8_lossy(&term.out[start..]).into_owned();
    assert_eq!(out.matches("\x1b]133;C").count(), 1, "{out:?}");
    assert!(
        !out.contains("MINE=0"),
        "the person's trap stopped running: {out:?}"
    );
}

/// Under a TERM with no terminal description, temor prints zsh's folder line itself, so a
/// screen clear (Ctrl-L) must print it again at the top or the person loses it.
#[test]
fn zsh_prints_the_folder_line_again_after_ctrl_l_under_an_unknown_term() {
    let test = "zsh_prints_the_folder_line_again_after_ctrl_l_under_an_unknown_term";
    if crate::support::pty_unsupported(test) {
        return;
    }
    let Some(zsh) = program_or_skip("zsh", test) else {
        return;
    };
    let scratch = Scratch::new();
    // An empty .zshrc keeps the stock prompt, which temor replaces, and the wizard away.
    std::fs::write(scratch.home.join(".zshrc"), "").expect(".zshrc");
    let mut env = scratch.contract_env(&ChildEnv::new());
    env.set("TERM", "temor-no-such-terminal");
    let spec = prepare_shell(
        &Shell::at(zsh),
        ShellStart::Interactive,
        env,
        &scratch.runtime,
        scratch.cwd.clone(),
        size(),
    )
    .expect("prepare shell");
    let mut term = Term::spawn(&spec, &setsid());
    assert!(term.read_until_has(0, PROMPT_END), "{:?}", term.text());
    assert!(term.text().contains("work dir"), "{:?}", term.text());
    let start = term.out.len();
    term.send("\x0c");
    assert!(
        term.read_until_has(start, "work dir"),
        "no folder line after the clear: {:?}",
        term.text()
    );
    let out = &term.out[start..];
    let clear = find(out, b"\x1b[2J").expect("the screen is cleared");
    let line = find(out, b"work dir").expect("folder line");
    assert!(clear < line, "{out:?}");
}
