//! nushell's integration: its own marks are switched on from a vendor autoload file in temor's
//! folder, and that folder is taken back out of `XDG_DATA_DIRS` once nushell has read it, so the
//! value a program sees is the value the person's environment had.

use temor_pty::{ChildEnv, Shell, ShellStart, prepare_shell};

use crate::support::{Scratch, Term, program_or_skip, pty_unsupported, setsid, size};

/// The value the person's environment starts with (nushell must hand it back unchanged).
const ORIGINAL_XDG: &str = "/opt/temor-test-xdg";

#[test]
fn nushell_marks_its_commands_and_gives_xdg_data_dirs_back_unchanged() {
    if pty_unsupported("nushell_marks_its_commands_and_gives_xdg_data_dirs_back_unchanged") {
        return;
    }
    let Some(nu) = program_or_skip("nu", "nushell_env") else {
        return;
    };
    let scratch = Scratch::new();
    let mut profile = ChildEnv::new();
    profile.set("XDG_DATA_DIRS", ORIGINAL_XDG);
    let spec = prepare_shell(
        &Shell::at(nu),
        ShellStart::Interactive,
        scratch.contract_env(&profile),
        &scratch.runtime,
        scratch.cwd.clone(),
        size(),
    )
    .expect("prepare shell");
    let mut term = Term::spawn(&spec, &setsid());
    assert!(
        term.read_until_has(0, "\x1b]133;B"),
        "no prompt mark: {:?}",
        term.text()
    );
    let start = term.out.len();
    term.send(
        "print $\"xdg=[($env | get -o XDG_DATA_DIRS)]\"; print ('end-' + (1 + 1 | into string))\r",
    );
    assert!(term.read_until_has(start, "end-2"), "{:?}", term.text());
    let text = String::from_utf8_lossy(&term.out[start..]).into_owned();
    assert!(
        text.contains(&format!("xdg=[{ORIGINAL_XDG}]")),
        "XDG_DATA_DIRS is the person's own value: {text:?}"
    );

    // A command runs between a start mark (C) and its end with status 0 (D;0).
    let marks = term.out.len();
    term.send("print ('ran-' + 'ok')\r");
    assert!(
        term.read_until_has(marks, "\x1b]133;C"),
        "no command start: {:?}",
        term.text()
    );
    assert!(
        term.read_until_has(marks, "\x1b]133;D;0"),
        "no command end: {:?}",
        term.text()
    );
    let all = String::from_utf8_lossy(&term.out).into_owned();
    assert!(all.contains("\x1b]133;A"), "no prompt start mark");
}
