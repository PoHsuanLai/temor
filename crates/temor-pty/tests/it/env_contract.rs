//! The child's environment is exactly the one built: the base, the contract and the profile.

use std::collections::BTreeSet;

use temor_pty::{
    ChildEnv, EnvContract, PaneId, ProgramVersion, SpawnSpec, TermName, base_env, child_env,
    find_term, terminfo_dirs,
};

use crate::support::{Scratch, Term, program, size, trampoline};

#[test]
fn env_holds_the_contract_and_nothing_inherited() {
    if crate::support::pty_unsupported("env_holds_the_contract_and_nothing_inherited") {
        return;
    }
    let scratch = Scratch::new();
    let terminfo = scratch.home.join("terminfo");
    std::fs::create_dir_all(terminfo.join("x")).expect("terminfo dir");
    std::fs::write(terminfo.join("x/xterm-ghostty"), b"").expect("terminfo entry");

    // What the app would pass from its own environment, as an injected source.
    let source = [
        ("PATH", "/usr/bin:/bin"),
        ("HOME", scratch.home.to_str().expect("utf-8 home")),
        ("TERM", "xterm-kitty"),
        ("KITTY_WINDOW_ID", "4"),
        ("TEMOR_PANE_ID", "stale"),
        ("TERMINFO", terminfo.to_str().expect("utf-8 terminfo")),
    ];
    let base = base_env(source);
    let term = find_term(&terminfo_dirs(&base, &[]));
    assert_eq!(term, TermName::XtermGhostty);
    let without = find_term(&terminfo_dirs(&ChildEnv::new(), &[]));
    assert_eq!(without, TermName::Xterm256Color);

    let contract = EnvContract {
        term,
        version: ProgramVersion("9.9.9".into()),
        pane: PaneId("pane-42".into()),
        socket: Some(scratch.runtime.join("temor.sock")),
    };
    let profile: ChildEnv = [("SPACE_TOKEN", "abc")].into_iter().collect();
    let env = child_env(base, &profile, &contract);
    let spec = SpawnSpec {
        program: program("env").expect("env on PATH"),
        args: Vec::new(),
        cwd: scratch.cwd.clone(),
        env: env.clone(),
        size: size(),
    };
    let mut term = Term::spawn(&spec, &trampoline());
    assert!(term.read_to_close(), "env did not finish");

    let printed: BTreeSet<String> = term
        .text()
        .lines()
        .map(|line| line.trim_end_matches('\r').to_owned())
        .filter(|line| !line.is_empty())
        .collect();
    let socket = scratch.runtime.join("temor.sock");
    let want: BTreeSet<String> = [
        "PATH=/usr/bin:/bin".to_owned(),
        format!("HOME={}", scratch.home.display()),
        format!("TERMINFO={}", terminfo.display()),
        "TERM=xterm-ghostty".to_owned(),
        "COLORTERM=truecolor".to_owned(),
        "SPACE_TOKEN=abc".to_owned(),
        "TERM_PROGRAM=temor".to_owned(),
        "TERM_PROGRAM_VERSION=9.9.9".to_owned(),
        "TEMOR_PANE_ID=pane-42".to_owned(),
        format!("TEMOR_SOCKET={}", socket.display()),
    ]
    .into_iter()
    .collect();
    assert_eq!(printed, want);
}
