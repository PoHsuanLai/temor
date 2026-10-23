//! Shell state reports: each shell says what its script is able to say (its adapter's `caps`),
//! and the session's `ShellState` follows what the person does in it.

use temor_core::block::Outcome;
use temor_core::session::EnvEntry;
use temor_shell::{ShellKind, adapter_for};

use super::super::support::window_tools_present as tools_present;
use super::support::{pump, run_line, shells, start};

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
        run_line(
            &live,
            "export TEMOR_CASE_VAR=kept-value MY_API_TOKEN=top-secret; alias tzz='echo zz'; cd /",
        );
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
