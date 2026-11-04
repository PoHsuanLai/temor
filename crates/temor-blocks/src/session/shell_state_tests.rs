//! The shell's state from its reports: what is kept, and what a new shell is told.

use proptest::prelude::*;
use temor_shell::report::Report;

use super::{
    EnvEntry, Replay, ShellIn, ShellState, holds_secret, is_secret, looks_secret, never_carried,
};

fn report(env: &[(&str, &str)], aliases: &[(&str, &str)]) -> ShellIn {
    let own = |items: &[(&str, &str)]| {
        items
            .iter()
            .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
            .collect()
    };
    ShellIn::Reported(Report {
        key: String::new(),
        env: own(env),
        aliases: own(aliases),
    })
}

fn value(text: &str) -> EnvEntry {
    EnvEntry::Value(text.to_owned())
}

/// The state after taking in each of `inputs`, from nothing.
fn applied(inputs: impl IntoIterator<Item = ShellIn>) -> ShellState {
    let mut state = ShellState::default();
    for input in inputs {
        state.apply(input);
    }
    state
}

#[test]
fn each_report_replaces_the_variables_and_aliases() {
    let mut state = applied([report(
        &[("EDITOR", "vim"), ("LANG", "C")],
        &[("ll", "ls -l")],
    )]);
    let env = |state: &ShellState| state.env().clone().into_iter().collect::<Vec<_>>();
    assert_eq!(
        env(&state),
        [("EDITOR".into(), value("vim")), ("LANG".into(), value("C"))]
    );
    state.apply(report(&[("EDITOR", "nvim"), ("PAGER", "less")], &[]));
    assert_eq!(
        env(&state),
        [
            ("EDITOR".into(), value("nvim")),
            ("PAGER".into(), value("less"))
        ]
    );
    assert!(
        state.aliases().is_empty(),
        "an alias the report leaves out is gone"
    );
}

#[test]
fn the_folder_is_state_too() {
    assert_eq!(applied([ShellIn::Cwd("/a".into())]).cwd(), Some("/a"));
}

#[test]
fn a_secret_is_a_name_only_and_is_never_replayed() {
    let secrets = [
        "GITHUB_TOKEN",
        "AWS_SECRET_ACCESS_KEY",
        "DB_PASSWORD",
        "STRIPE_API_KEY",
        "OPENAI_API_KEY",
        "SSH_PRIVATE_KEY",
        "NPM_AUTH",
        "my_key",
        "DB_PASS",
        "MYSQL_PWD",
        "GITLAB_PAT",
        "SENTRY_DSN",
        "MAPS_APIKEY",
        "GOOGLE_API_KEY",
        "AUTH_HEADER",
        "AZURE_CREDENTIALS",
        "PRIVATE_THING",
    ];
    for name in secrets {
        assert!(is_secret(name), "{name}");
    }
    let plain = [
        "EDITOR",
        "KEYBOARD",
        "HOME",
        "MONKEY_PATCH",
        "TERM",
        "MY_PATH",
        "GIT_AUTHOR_NAME",
        "PASSENGER",
        "OLDPWD",
        "PATTERN",
    ];
    for name in plain {
        assert!(!is_secret(name), "{name}");
    }
    let state = applied([report(
        &[("EDITOR", "vim"), ("GITHUB_TOKEN", "ghp_abc")],
        &[],
    )]);
    assert_eq!(state.env().get("GITHUB_TOKEN"), Some(&EnvEntry::Secret));
    let replay = state.replay();
    assert_eq!(replay.env, [("EDITOR".to_owned(), "vim".to_owned())]);
    assert_eq!(replay.withheld, ["GITHUB_TOKEN"]);
    assert!(
        !format!("{state:?}").contains("ghp_abc"),
        "the value is nowhere in the state"
    );
}

#[test]
fn what_the_shell_sets_for_itself_is_not_carried() {
    let state = applied([report(
        &[
            ("PWD", "/x"),
            ("SHLVL", "2"),
            ("ZDOTDIR", "/z"),
            ("TEMOR_PROMPT", "1"),
            ("A", "1"),
        ],
        &[],
    )]);
    assert_eq!(state.replay().env, [("A".to_owned(), "1".to_owned())]);
}

#[test]
fn the_replay_is_the_folder_the_variables_and_the_aliases() {
    let state = applied([
        ShellIn::Cwd("/work".into()),
        report(&[("A", "1")], &[("g", "git")]),
    ]);
    assert_eq!(
        state.replay(),
        Replay {
            cwd: Some("/work".into()),
            env: vec![("A".into(), "1".into())],
            aliases: vec![("g".into(), "git".into())],
            withheld: vec![],
        }
    );
}

proptest! {
    /// Folding the reports one by one or only the last one gives the same state: a report is a
    /// snapshot, so earlier ones do not matter.
    #[test]
    fn the_last_report_decides_the_state(
        reports in proptest::collection::vec(
            proptest::collection::vec(("[A-Z]{1,5}", "[a-z]{0,5}"), 0..5), 1..5),
    ) {
        let ins: Vec<ShellIn> = reports
            .iter()
            .map(|env| ShellIn::Reported(Report { key: String::new(), env: env.clone(), aliases: vec![] }))
            .collect();
        let all = applied(ins.clone());
        let last = applied(ins.last().cloned());
        prop_assert_eq!(all, last);
    }
}

#[test]
fn a_state_restored_from_a_replay_replays_the_same_and_keeps_secrets_as_names() {
    let replay = Replay {
        cwd: Some("/tmp".into()),
        env: vec![("FOO".into(), "bar".into())],
        aliases: vec![("zz".into(), "echo zed".into())],
        withheld: vec!["MY_TOKEN".into()],
    };
    let state = ShellState::restored(&replay);
    assert_eq!(state.replay(), replay);
    assert_eq!(state.env().get("MY_TOKEN"), Some(&EnvEntry::Secret));
}

#[test]
fn a_value_that_looks_like_a_secret_is_one_whatever_its_name() {
    let secret = [
        "postgres://app:hunter2@db.local:5432/app",
        "https://user:pw@example.com",
        concat!("ghp_", "0123456789abcdefABCDEF"),
        concat!("gho_", "0123456789abcdefABCDEF"),
        concat!("ghs_", "0123456789abcdefABCDEF"),
        concat!("ghu_", "0123456789abcdefABCDEF"),
        concat!("github_pat_", "11ABCDEFG0123456789_abc"),
        concat!("glpat-", "0123456789abcdefABCD"),
        concat!("sk-proj-", "0123456789abcdefABCD"),
        concat!("xoxb-", "1234-5678-abcdefghijkl"),
        concat!("xoxp-", "1234-5678-abcdefghijkl"),
        concat!("AKIA", "IOSFODNN7EXAMPLE"),
        concat!("ASIA", "IOSFODNN7EXAMPLE"),
        concat!("Bearer ghp_", "0123456789abcdefABCDEF"),
    ];
    for value in secret {
        assert!(looks_secret(value), "{value}");
    }
    let plain = [
        "https://example.com/a:b@c",
        "git@github.com:me/repo.git",
        "ssh://git@host/repo",
        "sk-feature",
        "ghp_short",
        "AKIA",
        "/usr/bin:/bin",
        "vim",
    ];
    for value in plain {
        assert!(!looks_secret(value), "{value}");
    }
    let state = applied([report(&[("DATABASE_URL", "postgres://u:pw@db/app")], &[])]);
    assert_eq!(state.env().get("DATABASE_URL"), Some(&EnvEntry::Secret));
}

#[test]
fn an_alias_that_holds_a_secret_is_not_kept() {
    assert!(holds_secret("export GITHUB_TOKEN=abc && gh pr list"));
    assert!(holds_secret("DB_PASSWORD=x psql"));
    assert!(holds_secret("curl -u me:pw https://me:pw@host/api"));
    assert!(holds_secret(concat!(
        "gh --token ghp_",
        "0123456789abcdefABCDEF"
    )));
    assert!(!holds_secret("ls -la"));
    assert!(!holds_secret("echo $GITHUB_TOKEN"));
    assert!(!holds_secret("git log --author=me"));
    let state = applied([report(
        &[],
        &[
            ("ll", "ls -l"),
            ("deploy", "AWS_SECRET_ACCESS_KEY=abc ./deploy"),
            ("gh2", concat!("gh --token ghp_", "0123456789abcdefABCDEF")),
        ],
    )]);
    assert_eq!(
        state.replay().aliases,
        [("ll".to_owned(), "ls -l".to_owned())]
    );
    assert!(!format!("{state:?}").contains("ghp_"));
}

#[test]
fn what_names_the_session_or_loads_code_is_never_carried() {
    let never = [
        "PATH",
        "MANPATH",
        "INFOPATH",
        "FPATH",
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
        "DYLD_INSERT_LIBRARIES",
        "BASH_ENV",
        "ENV",
        "ZDOTDIR",
        "PROMPT_COMMAND",
        "PS1",
        "PS4",
        "HOME",
        "SHELL",
        "USER",
        "LOGNAME",
        "SHLVL",
        "TERM",
        "TERM_PROGRAM",
        "TERM_PROGRAM_VERSION",
        "COLORTERM",
        "TMUX",
        "TMUX_PANE",
        "STY",
        "WINDOWID",
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "XAUTHORITY",
        "SSH_AUTH_SOCK",
        "SSH_CONNECTION",
        "SSH_CLIENT",
        "SSH_TTY",
        "DBUS_SESSION_BUS_ADDRESS",
        "XDG_RUNTIME_DIR",
        "XDG_SESSION_TYPE",
        "TEMOR_REPLAY",
        "KITTY_WINDOW_ID",
        "GHOSTTY_RESOURCES_DIR",
    ];
    for name in never {
        assert!(never_carried(name), "{name}");
    }
    for name in [
        "EDITOR",
        "GOPATH",
        "PAGER",
        "LANG",
        "XDG_CONFIG_HOME",
        "TERMINFO",
    ] {
        assert!(!never_carried(name), "{name}");
    }
    let reported: Vec<(&str, &str)> = never.iter().map(|name| (*name, "x")).collect();
    let state = applied([report(&reported, &[])]);
    assert_eq!(state.replay().env, []);
}
