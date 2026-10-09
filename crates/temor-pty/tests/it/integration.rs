//! The OSC 133 and OSC 7 marks each shell's integration emits around `true` and `false`.

use std::path::PathBuf;

use temor_pty::prepare_shell;
use temor_shell::{ChildEnv, Shell, ShellStart};

use crate::support::{Scratch, Term, find, program_or_skip, setsid, size, url_path};

// fish 4 adds options (`A;click_events=1`, `C;cmdline_url=…`), so A and C match as prefixes.
const PROMPT_START: &str = "\x1b]133;A";
const PROMPT_END: &str = "\x1b]133;B\x1b\\";
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
        let done = format!("\x1b]133;D;{status}\x1b\\");
        assert!(
            term.read_until_has(start, &done) && term.read_until_has(start, PROMPT_END),
            "{command}: no D;{status} and prompt: {:?}",
            term.text()
        );
        let out = &term.out[start..];
        let c = find(out, OUTPUT_START.as_bytes()).expect("C");
        let d = find(out, done.as_bytes()).expect("D");
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
