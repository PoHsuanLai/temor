//! What a pane's shell holds that a new shell would need to look the same: its folder, exported
//! variables and aliases. Built from the shell's own reports, one step at a time, so the same
//! reports in the same order give the same state; a change is said as a [`ShellDelta`] and the
//! whole state can be turned into what a fresh shell must be told ([`ShellState::replay`]).
//!
//! A variable whose name says it is a secret (`*_TOKEN`, `*_KEY`, `*_SECRET`, `*PASSWORD*`,
//! credentials) is kept as a name only: its value never enters the state, so it is never copied,
//! saved or replayed.

use std::collections::BTreeMap;

use temor_shell::report::Report;

/// A variable's entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvEntry {
    Value(String),
    /// The shell has it, and its value is not kept.
    Secret,
}

/// The state of a shell.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShellState {
    cwd: Option<String>,
    env: BTreeMap<String, EnvEntry>,
    aliases: BTreeMap<String, String>,
}

/// What changed between two states.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShellDelta {
    pub cwd: Option<String>,
    pub env_set: Vec<(String, EnvEntry)>,
    pub env_unset: Vec<String>,
    pub alias_set: Vec<(String, String)>,
    pub alias_unset: Vec<String>,
}

impl ShellDelta {
    /// Whether nothing changed.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

/// What reaches the shell's state.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ShellIn {
    /// The shell's folder (OSC 7).
    Cwd(String),
    /// The shell's own report of its variables and aliases.
    Reported(Report),
}

/// What a step says.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ShellOut {
    Changed(ShellDelta),
}

/// What a fresh shell must be told to be like the one recorded: no secrets, no variables the
/// new shell sets for itself.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Replay {
    pub cwd: Option<String>,
    pub env: Vec<(String, String)>,
    pub aliases: Vec<(String, String)>,
    /// Names the old shell had that are not copied because they are secrets.
    pub withheld: Vec<String>,
}

/// Variables a shell or temor sets for itself: not state to carry.
const OWN: &[&str] = &[
    "_",
    "PWD",
    "OLDPWD",
    "SHLVL",
    "ZDOTDIR",
    "COLUMNS",
    "LINES",
    "TERM_SESSION_ID",
    "TERM_PROGRAM_VERSION",
    "SSH_AUTH_SOCK",
    "SSH_CONNECTION",
    "SSH_TTY",
    "TMPDIR",
    "XDG_RUNTIME_DIR",
    "DISPLAY",
    "WAYLAND_DISPLAY",
    "DBUS_SESSION_BUS_ADDRESS",
    "XPC_FLAGS",
    "XPC_SERVICE_NAME",
    "SECURITYSESSIONID",
    "__CF_USER_TEXT_ENCODING",
];

fn is_own(name: &str) -> bool {
    OWN.contains(&name) || name.starts_with("TEMOR_")
}

/// Whether a variable's name says its value is a secret.
pub fn is_secret(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    [
        "TOKEN",
        "SECRET",
        "PASSWORD",
        "PASSWD",
        "PASSPHRASE",
        "CREDENTIAL",
        "PRIVATE_KEY",
        "API_KEY",
        "ACCESS_KEY",
    ]
    .iter()
    .any(|word| upper.contains(word))
        || upper.ends_with("_KEY")
        || upper.ends_with("_AUTH")
        || upper == "AWS_SESSION_TOKEN"
}

impl ShellState {
    pub fn cwd(&self) -> Option<&str> {
        self.cwd.as_deref()
    }

    pub fn env(&self) -> &BTreeMap<String, EnvEntry> {
        &self.env
    }

    pub fn aliases(&self) -> &BTreeMap<String, String> {
        &self.aliases
    }

    /// The state of a shell that is known only by what was kept of it (a checkpoint): the shell
    /// itself says nothing until something it holds changes.
    pub fn restored(replay: &Replay) -> Self {
        let mut env: BTreeMap<String, EnvEntry> = replay
            .env
            .iter()
            .map(|(name, value)| (name.clone(), EnvEntry::Value(value.clone())))
            .collect();
        env.extend(
            replay
                .withheld
                .iter()
                .map(|name| (name.clone(), EnvEntry::Secret)),
        );
        Self {
            cwd: replay.cwd.clone(),
            env,
            aliases: replay.aliases.iter().cloned().collect(),
        }
    }

    /// What to tell a fresh shell.
    pub fn replay(&self) -> Replay {
        let mut replay = Replay {
            cwd: self.cwd.clone(),
            aliases: self
                .aliases
                .iter()
                .map(|(name, value)| (name.clone(), value.clone()))
                .collect(),
            ..Replay::default()
        };
        for (name, entry) in &self.env {
            match entry {
                EnvEntry::Value(value) => replay.env.push((name.clone(), value.clone())),
                EnvEntry::Secret => replay.withheld.push(name.clone()),
            }
        }
        replay
    }
}

fn entry_of(name: &str, value: &str) -> EnvEntry {
    match is_secret(name) {
        true => EnvEntry::Secret,
        false => EnvEntry::Value(value.to_owned()),
    }
}

/// The state after `input`, and what changed.
pub fn shell_step(mut state: ShellState, input: ShellIn) -> (ShellState, Vec<ShellOut>) {
    let mut delta = ShellDelta::default();
    match input {
        ShellIn::Cwd(path) => {
            if state.cwd.as_deref() != Some(path.as_str()) {
                state.cwd = Some(path.clone());
                delta.cwd = Some(path);
            }
        }
        ShellIn::Reported(report) => {
            let env: BTreeMap<String, EnvEntry> = report
                .env
                .iter()
                .filter(|(name, _)| !is_own(name))
                .map(|(name, value)| (name.clone(), entry_of(name, value)))
                .collect();
            let aliases: BTreeMap<String, String> = report.aliases.into_iter().collect();
            for (name, entry) in &env {
                if state.env.get(name) != Some(entry) {
                    delta.env_set.push((name.clone(), entry.clone()));
                }
            }
            delta.env_unset = state
                .env
                .keys()
                .filter(|name| !env.contains_key(*name))
                .cloned()
                .collect();
            for (name, value) in &aliases {
                if state.aliases.get(name) != Some(value) {
                    delta.alias_set.push((name.clone(), value.clone()));
                }
            }
            delta.alias_unset = state
                .aliases
                .keys()
                .filter(|name| !aliases.contains_key(*name))
                .cloned()
                .collect();
            state.env = env;
            state.aliases = aliases;
        }
    }
    let outs = match delta.is_empty() {
        true => Vec::new(),
        false => vec![ShellOut::Changed(delta)],
    };
    (state, outs)
}

#[cfg(test)]
#[path = "shell_state_tests.rs"]
mod tests;
