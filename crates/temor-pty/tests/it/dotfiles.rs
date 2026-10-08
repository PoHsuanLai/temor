//! The bash shim runs the person's own startup files first, and writes nothing to HOME.

use std::collections::BTreeSet;

use temor_pty::{ChildEnv, Shell, ShellStart, prepare_shell};

use crate::support::{Scratch, Term, program, setsid, size};

fn home_entries(scratch: &Scratch) -> BTreeSet<String> {
    std::fs::read_dir(&scratch.home)
        .expect("home")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        // bash saves history there when hung up, and the system's /etc/profile.d scripts may
        // make ~/.cache in a login shell: the shell's doing, not temor's.
        .filter(|name| name != ".bash_history" && name != ".cache")
        .collect()
}

fn run(start: ShellStart, check: &str) {
    let scratch = Scratch::new();
    let home = &scratch.home;
    std::fs::write(
        home.join(".bashrc"),
        "echo bashrc-ran\nFROM_RC=rc-value\nPS1='rc$ '\n",
    )
    .expect(".bashrc");
    std::fs::write(
        home.join(".bash_profile"),
        "echo profile-ran\nFROM_RC=profile-value\nPS1='login$ '\n",
    )
    .expect(".bash_profile");
    let before = home_entries(&scratch);
    let spec = prepare_shell(
        &Shell::at(program("bash").expect("bash on PATH")),
        start,
        scratch.contract_env(&ChildEnv::new()),
        &scratch.runtime,
        scratch.cwd.clone(),
        size(),
    )
    .expect("prepare shell");
    let mut term = Term::spawn(&spec, &setsid());
    assert!(
        term.read_until_has(0, "\x1b]133;B"),
        "no marked prompt: {:?}",
        term.text()
    );
    let start_at = term.out.len();
    term.send("echo \"got=$FROM_RC login=${TEMOR_BASH_LOGIN-unset}\"\r");
    assert!(
        term.read_until_has(start_at, &format!("got={check} login=unset")),
        "{:?}",
        term.text()
    );
    assert!(
        term.read_until_has(start_at, "\x1b]133;D;0"),
        "{:?}",
        term.text()
    );
    drop(term);
    assert_eq!(home_entries(&scratch), before, "HOME was written to");
}

#[test]
fn bash_shim_sources_bashrc_first() {
    run(ShellStart::Interactive, "rc-value");
}

#[test]
fn bash_login_shim_sources_bash_profile_first() {
    run(ShellStart::Login, "profile-value");
}
