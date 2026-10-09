use std::path::PathBuf;
use temor_shell::ChildEnv;

use super::*;

fn env(pairs: &[(&str, &str)]) -> ChildEnv {
    pairs.iter().copied().collect()
}

#[test]
fn base_env_drops_other_terminals_and_keeps_the_rest() {
    let base = base_env([
        ("PATH", "/usr/bin"),
        ("TERM", "xterm-kitty"),
        ("KITTY_WINDOW_ID", "3"),
        ("TEMOR_PANE_ID", "old"),
        ("TEMOR_SHELL_INTEGRATION", "0"),
        ("COLUMNS", "80"),
        ("LANG", "C.UTF-8"),
    ]);
    assert_eq!(
        base,
        env(&[
            ("PATH", "/usr/bin"),
            ("TEMOR_SHELL_INTEGRATION", "0"),
            ("LANG", "C.UTF-8")
        ])
    );
}

#[test]
fn profile_overrides_term_but_not_identity() {
    let contract = EnvContract {
        term: TermName::XtermGhostty,
        version: ProgramVersion("1.2.3".into()),
        pane: PaneId("p7".into()),
        socket: None,
    };
    let profile = env(&[
        ("TERM", "xterm-256color"),
        ("TEMOR_PANE_ID", "spoof"),
        ("TEMOR_SOCKET", "/spoof"),
        ("API", "x"),
    ]);
    let out = child_env(env(&[("HOME", "/h")]), &profile, &contract);
    assert_eq!(
        out,
        env(&[
            ("HOME", "/h"),
            ("TERM", "xterm-256color"),
            ("COLORTERM", "truecolor"),
            ("API", "x"),
            ("TERM_PROGRAM", "temor"),
            ("TERM_PROGRAM_VERSION", "1.2.3"),
            ("TEMOR_PANE_ID", "p7"),
        ])
    );
}

#[test]
fn terminfo_dirs_follow_ncurses_order() {
    const SYSTEM: &[&str] = &["/sys1", "/sys2"];
    type Vars = &'static [(&'static str, &'static str)];
    const CASES: &[(Vars, &[&str])] = &[
        (&[], &["/sys1", "/sys2"]),
        (&[("HOME", "/h")], &["/h/.terminfo", "/sys1", "/sys2"]),
        (&[("TERMINFO", "/t")], &["/t", "/sys1", "/sys2"]),
        (
            &[("TERMINFO_DIRS", "/a::/b")],
            &["/a", "/sys1", "/sys2", "/b"],
        ),
        (&[("TERMINFO_DIRS", "/a")], &["/a"]),
    ];
    for (vars, want) in CASES {
        let want: Vec<PathBuf> = want.iter().map(PathBuf::from).collect();
        assert_eq!(terminfo_dirs(&env(vars), SYSTEM), want, "{vars:?}");
    }
}

#[test]
fn find_term_falls_back_without_an_entry() {
    let dir = std::env::temp_dir().join(format!("temor-pty-terminfo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(
        find_term(std::slice::from_ref(&dir)),
        TermName::Xterm256Color
    );
    std::fs::create_dir_all(dir.join("78")).unwrap();
    std::fs::write(dir.join("78").join("xterm-ghostty"), b"").unwrap();
    assert_eq!(
        find_term(std::slice::from_ref(&dir)),
        TermName::XtermGhostty
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
