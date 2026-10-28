//! The shell's state from its reports: what is kept, what changed, and what a new shell is told.

use proptest::prelude::*;
use temor_shell::report::Report;

use super::{
    EnvEntry, Replay, ShellDelta, ShellIn, ShellOut, ShellState, holds_secret, is_secret,
    looks_secret, never_carried, shell_step,
};
use temor_testkit::run_steps;

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

#[test]
fn a_report_says_what_was_added_changed_and_removed() {
    let (state, outs) = shell_step(
        ShellState::default(),
        report(&[("EDITOR", "vim"), ("LANG", "C")], &[("ll", "ls -l")]),
    );
    assert_eq!(
        outs,
        [ShellOut::Changed(ShellDelta {
            env_set: vec![("EDITOR".into(), value("vim")), ("LANG".into(), value("C"))],
            alias_set: vec![("ll".into(), "ls -l".into())],
            ..ShellDelta::default()
        })]
    );
    let (state, outs) = shell_step(
        state,
        report(&[("EDITOR", "nvim"), ("PAGER", "less")], &[("ll", "ls -l")]),
    );
    assert_eq!(
        outs,
        [ShellOut::Changed(ShellDelta {
            env_set: vec![
                ("EDITOR".into(), value("nvim")),
                ("PAGER".into(), value("less"))
            ],
            env_unset: vec!["LANG".into()],
            ..ShellDelta::default()
        })]
    );
    // The same report again changes nothing and says nothing.
    let (_, outs) = shell_step(
        state.clone(),
        report(&[("EDITOR", "nvim"), ("PAGER", "less")], &[("ll", "ls -l")]),
    );
    assert_eq!(outs, []);
}

#[test]
fn the_folder_is_state_too() {
    let (state, outs) = shell_step(ShellState::default(), ShellIn::Cwd("/a".into()));
    assert_eq!(state.cwd(), Some("/a"));
    assert_eq!(outs.len(), 1);
    let (_, outs) = shell_step(state, ShellIn::Cwd("/a".into()));
    assert_eq!(outs, [], "the same folder is no change");
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
    let (state, _) = shell_step(
        ShellState::default(),
        report(&[("EDITOR", "vim"), ("GITHUB_TOKEN", "ghp_abc")], &[]),
    );
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
    let (state, _) = shell_step(
        ShellState::default(),
        report(
            &[
                ("PWD", "/x"),
                ("SHLVL", "2"),
                ("ZDOTDIR", "/z"),
                ("TEMOR_PROMPT", "1"),
                ("A", "1"),
            ],
            &[],
        ),
    );
    assert_eq!(state.replay().env, [("A".to_owned(), "1".to_owned())]);
}

#[test]
fn the_replay_is_the_folder_the_variables_and_the_aliases() {
    let (state, _) = run_steps(
        ShellState::default(),
        [
            ShellIn::Cwd("/work".into()),
            report(&[("A", "1")], &[("g", "git")]),
        ],
        shell_step,
    );
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
        let (all, _) = run_steps(ShellState::default(), ins.clone(), shell_step);
        let (last, _) = run_steps(ShellState::default(), vec![ins.last().cloned().unwrap()], shell_step);
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
    let (state, _) = shell_step(
        ShellState::default(),
        report(&[("DATABASE_URL", "postgres://u:pw@db/app")], &[]),
    );
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
    let (state, _) = shell_step(
        ShellState::default(),
        report(
            &[],
            &[
                ("ll", "ls -l"),
                ("deploy", "AWS_SECRET_ACCESS_KEY=abc ./deploy"),
                ("gh2", concat!("gh --token ghp_", "0123456789abcdefABCDEF")),
            ],
        ),
    );
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
    let (state, _) = shell_step(ShellState::default(), report(&reported, &[]));
    assert_eq!(state.replay().env, []);
}
