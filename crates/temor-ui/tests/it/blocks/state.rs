//! Shell state reports: each shell says what its script is able to say (its adapter's `caps`),
//! and the session's `ShellState` follows what the person does in it.

use temor_client::model::block::Outcome;
use temor_client::model::session::EnvEntry;
use temor_shell::{ShellKind, adapter_for};

use super::super::support::window_tools_present as tools_present;
use super::super::support::{scratch_path, window_tools_present};
use super::support::{pump, run_line, settle, shell_path, shells, start, start_with};

fn kind_of(name: &str) -> ShellKind {
    match name {
        "bash" => ShellKind::Bash,
        "zsh" => ShellKind::Zsh,
        "fish" => ShellKind::Fish,
        _ => ShellKind::Nushell,
    }
}

/// The conformance check, for every installed shell: a variable exported, an alias made and a
/// folder entered show up in the state (as far as the adapter says its script reports them), and
/// a secret's value does not.
#[test]
fn shells_report_what_their_adapters_say_they_can() {
    if !tools_present() {
        return;
    }
    let _alone = super::super::support::heavy();
    for (name, shell) in shells() {
        let caps = adapter_for(kind_of(name)).caps();
        if !caps.env && !caps.aliases {
            continue;
        }
        let mut live = start(&format!("state-{name}"), &shell);
        let line = match name {
            "nu" => "$env.TEMOR_CASE_VAR = 'kept-value'; $env.MY_API_TOKEN = 'top-secret'; cd /",
            _ => {
                "export TEMOR_CASE_VAR=kept-value MY_API_TOKEN=top-secret; alias tzz='echo zz'; cd /"
            }
        };
        run_line(&live, line);
        pump(&mut live, "the command", |s| {
            s.block_outcomes()
                .first()
                .is_some_and(|b| b.1 != Outcome::Running)
        });
        pump(&mut live, "the shell's report", |s| {
            let state = s.shell_state();
            let env_ok = !caps.env || state.env().contains_key("MY_API_TOKEN");
            let alias_ok = !caps.aliases || state.aliases().contains_key("tzz");
            env_ok && alias_ok && state.cwd() == Some("/")
        });
        let state = live.session.shell_state();
        if caps.env {
            // TEMOR_ names are the integration's own and are not carried; a plain variable is.
            assert_eq!(state.env().get("TEMOR_CASE_VAR"), None, "{name}");
            assert_eq!(
                state.env().get("MY_API_TOKEN"),
                Some(&EnvEntry::Secret),
                "{name}"
            );
            assert!(
                !format!("{state:?}").contains("top-secret"),
                "{name}: no secret value"
            );
        }
        if caps.aliases {
            assert_eq!(
                state.aliases().get("tzz").map(String::as_str),
                Some("echo zz"),
                "{name}"
            );
        }
        assert_eq!(state.cwd(), Some("/"), "{name}");
    }
}

/// A new shell is given the aliases of the one it replaces: the script it is pointed at is read
/// at its first prompt and removed, and the shell then reports the aliases as its own.
#[test]
fn a_new_zsh_defines_the_aliases_it_is_given_once() {
    let Some(zsh) = shell_path("zsh").filter(|_| window_tools_present()) else {
        return;
    };
    let _alone = super::super::support::heavy();
    let script = temor_shell::alias_script(&[
        ("handed".to_owned(), "echo it's handed".to_owned()),
        ("bad name".to_owned(), "x".to_owned()),
    ]);
    let mut live = start_with("state-replay", &zsh, |home| {
        std::fs::write(home.join("replay.zsh"), &script).expect("script");
    });
    pump(&mut live, "the alias in the report", |s| {
        s.shell_state().aliases().contains_key("handed")
    });
    let state = live.session.shell_state();
    assert_eq!(
        state.aliases().get("handed").map(String::as_str),
        Some("echo it's handed")
    );
    assert!(!state.aliases().contains_key("bad name"));
    let home = scratch_path("state-replay").join("home");
    assert!(!home.join("replay.zsh").exists(), "read once, then removed");
}

/// Only the shell temor started can report: a report printed by a program (here `printf`, as a
/// file shown with `cat` would be) with another key or none changes nothing, the key is not in
/// the environment programs see, and a shell started inside the pane, even one that loads
/// temor's scripts, has no key and says nothing.
#[test]
fn only_the_shell_temor_started_reports() {
    if !tools_present() {
        return;
    }
    let _alone = super::super::support::heavy();
    for (name, shell) in shells() {
        let caps = adapter_for(kind_of(name)).caps();
        if !caps.env || !matches!(name, "bash" | "zsh") {
            continue;
        }
        let mut live = start(&format!("state-key-{name}"), &shell);
        run_line(&live, "export REAL_VAR=one");
        pump(&mut live, "the shell's report", |s| {
            s.shell_state().env().contains_key("REAL_VAR")
        });
        let forged = |key: &str| {
            format!(
                "printf '\\033]7503;v2;{key}e=REAL_VAR=forged;e=FORGED_VAR=1;a=forged_alias=curl%20x\\033\\\\'"
            )
        };
        let line = format!(
            "{}; {}; test -z \"$(printenv TEMOR_REPORT_KEY)\"",
            forged("k=0123456789abcdef0123456789abcdef;"),
            forged("")
        );
        run_line(&live, &line);
        pump(&mut live, "the forged reports", |s| {
            s.block_outcomes()
                .last()
                .is_some_and(|b| b.1 != Outcome::Running)
        });
        let finished = live.session.block_outcomes();
        assert_eq!(
            finished.last().map(|b| b.1),
            Some(Outcome::Succeeded),
            "{name}: a program sees no key"
        );
        // A nested shell with temor's scripts: it has no key, so what it sets is not reported.
        let nested = match name {
            "bash" => format!(
                "'{}' --rcfile \"$HOME/runtime/shell-integration/bash/temor-bashrc\" -i",
                shell.display()
            ),
            _ => format!(
                "ZDOTDIR=\"$HOME/runtime/shell-integration/zsh\" '{}' -i",
                shell.display()
            ),
        };
        run_line(&live, &nested);
        run_line(&live, "export NESTED_VAR=1; echo nested-$((40+1))");
        pump(&mut live, "the nested shell's command", |s| {
            s.screen_text().contains("nested-41")
        });
        run_line(&live, "exit");
        run_line(&live, "echo outer-$((40+2))");
        pump(&mut live, "the outer shell again", |s| {
            s.screen_text().contains("outer-42")
        });
        settle(&mut live, std::time::Duration::from_millis(300));
        let state = live.session.shell_state();
        assert_eq!(
            state.env().get("REAL_VAR"),
            Some(&EnvEntry::Value("one".to_owned())),
            "{name}"
        );
        assert!(!state.env().contains_key("FORGED_VAR"), "{name}");
        assert!(
            !state.env().contains_key("NESTED_VAR"),
            "{name}: the nested shell"
        );
        assert!(!state.aliases().contains_key("forged_alias"), "{name}");
    }
}

/// A command that prints nothing has no line under its title: the next prompt's folder line is
/// not its output.
#[test]
fn a_cd_prints_nothing_and_its_card_says_nothing_under_the_command() {
    if !window_tools_present() {
        return;
    }
    let _alone = super::super::support::heavy();
    for (name, shell) in shells() {
        let mut live = start(&format!("state-cd-{name}"), &shell);
        run_line(&live, "cd /");
        pump(&mut live, "the cd", |s| {
            s.block_outcomes()
                .first()
                .is_some_and(|b| b.1 != Outcome::Running)
        });
        let (id, _) = live.session.block_outcomes()[0];
        assert_eq!(
            live.session.block_output(id).as_deref().map(str::trim),
            Some(""),
            "{name}"
        );
        assert_eq!(live.session.block_note(id), None, "{name}");
    }
}
