//! `TEMOR_SHELL_INTEGRATION=0` starts the shell plainly, with no marks and no shims.

use temor_pty::prepare_shell;
use temor_shell::{ChildEnv, Shell, ShellStart};

use crate::support::{Scratch, Term, program, setsid, size};

#[test]
fn opt_out_injects_nothing() {
    if crate::support::pty_unsupported("opt_out_injects_nothing") {
        return;
    }
    let scratch = Scratch::new();
    std::fs::write(scratch.home.join(".bashrc"), "PS1='$ '\n").expect(".bashrc");
    let profile: ChildEnv = [("TEMOR_SHELL_INTEGRATION", "0")].into_iter().collect();
    let spec = prepare_shell(
        &Shell::at(program("bash").expect("bash on PATH")),
        ShellStart::Interactive,
        scratch.contract_env(&profile),
        &scratch.runtime,
        scratch.cwd.clone(),
        size(),
    )
    .expect("prepare shell");
    let mut term = Term::spawn(&spec, &setsid());
    assert!(term.read_until_has(0, "$ "), "no prompt: {:?}", term.text());
    let start = term.out.len();
    term.send("echo done-$((40+2))\r");
    assert!(term.read_until_has(start, "done-42"), "{:?}", term.text());
    assert!(term.read_until_has(start + 1, "$ "), "{:?}", term.text());
    let text = term.text();
    assert!(
        !text.contains("\x1b]133;"),
        "marks despite opt-out: {text:?}"
    );
    assert!(!text.contains("\x1b]7;"), "OSC 7 despite opt-out: {text:?}");
    let shims = std::fs::read_dir(&scratch.runtime)
        .expect("runtime dir")
        .count();
    assert_eq!(shims, 0, "shims written despite opt-out");
}
