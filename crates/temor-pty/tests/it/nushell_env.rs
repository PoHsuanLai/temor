//! nushell's integration: its own marks are switched on from a vendor autoload file in temor's
//! folder, and that folder is taken back out of `XDG_DATA_DIRS` once nushell has read it.

use temor_pty::{ChildEnv, Shell, ShellStart, prepare_shell};

use crate::support::{Scratch, Term, program_or_skip, pty_unsupported, setsid, size};

#[test]
fn nushell_marks_prompts_and_takes_its_folder_back_out_of_xdg() {
    if pty_unsupported("nushell_marks_prompts_and_takes_its_folder_back_out_of_xdg") {
        return;
    }
    let Some(nu) = program_or_skip("nu", "nushell_env") else {
        return;
    };
    let scratch = Scratch::new();
    let spec = prepare_shell(
        &Shell::at(nu),
        ShellStart::Interactive,
        scratch.contract_env(&ChildEnv::new()),
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
    assert!(!text.contains("shell-integration"), "{text:?}");
}
