//! What a pane's shell holds that a new shell would need to look the same: its folder, exported
//! variables and aliases. Built from the shell's own reports, one step at a time, so the same
//! reports in the same order give the same state, and the whole state can be turned into what a
//! fresh shell must be told ([`ShellState::replay`]).
//!
//! A variable whose name says it is a secret (`*_TOKEN`, `*_KEY`, `*_SECRET`, `*PASSWORD*`,
//! credentials), or whose value looks like one (a URL with a password in it, a well-known token
//! prefix), is kept as a name only: its value never enters the state, so it is never copied,
//! saved or replayed. An alias that holds a secret is not kept at all.

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

/// What reaches the shell's state.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ShellIn {
    /// The shell's folder (OSC 7).
    Cwd(String),
    /// The shell's own report of its variables and aliases.
    Reported(Report),
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

/// Variables never carried to a new shell, by exact name: what a shell sets for itself, what
/// names this login, terminal or desktop session (the new shell gets its own), and what changes
/// the code a shell or program loads. `PATH` and its kin are here because the new shell's startup
/// files extend them again: a carried `PATH` would grow at every restore.
const NEVER_CARRIED: &[&str] = &[
    "_",
    "PWD",
    "OLDPWD",
    "SHLVL",
    "HOME",
    "SHELL",
    "USER",
    "LOGNAME",
    "PATH",
    "MANPATH",
    "INFOPATH",
    "FPATH",
    "BASH_ENV",
    "ENV",
    "ZDOTDIR",
    "PROMPT_COMMAND",
    "PS1",
    "PS2",
    "PS3",
    "PS4",
    "TERM",
    "COLORTERM",
    "COLUMNS",
    "LINES",
    "TERM_SESSION_ID",
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
    "TMPDIR",
    "XPC_FLAGS",
    "XPC_SERVICE_NAME",
    "SECURITYSESSIONID",
    "__CF_USER_TEXT_ENCODING",
];

/// Variables never carried, by the start of their name: the code loaders' (`LD_*`, `DYLD_*`),
/// temor's own and other terminals', and the desktop session's.
const NEVER_CARRIED_PREFIXES: &[&str] = &[
    "LD_",
    "DYLD_",
    "TEMOR_",
    "TERM_PROGRAM",
    "KITTY_",
    "GHOSTTY_",
    "XDG_SESSION_",
];

/// Whether a variable is one a new shell must not be given from an old one (see
/// [`NEVER_CARRIED`]).
pub fn never_carried(name: &str) -> bool {
    NEVER_CARRIED.contains(&name) || NEVER_CARRIED_PREFIXES.iter().any(|p| name.starts_with(p))
}

// The secret rules are the schema's: history keeps none either.
pub use temor_schema::secret::{holds_secret, is_secret, looks_secret};

impl ShellState {
    /// Takes in `input`: the folder, or a report that replaces the variables and aliases.
    pub fn apply(&mut self, input: ShellIn) {
        match input {
            ShellIn::Cwd(path) => self.cwd = Some(path),
            ShellIn::Reported(report) => {
                self.env = report
                    .env
                    .iter()
                    .filter(|(name, _)| !never_carried(name))
                    .map(|(name, value)| (name.clone(), entry_of(name, value)))
                    .collect();
                self.aliases = report
                    .aliases
                    .into_iter()
                    .filter(|(_, value)| !holds_secret(value))
                    .collect();
            }
        }
    }

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
    match is_secret(name) || looks_secret(value) {
        true => EnvEntry::Secret,
        false => EnvEntry::Value(value.to_owned()),
    }
}

#[cfg(test)]
#[path = "shell_state_tests.rs"]
mod tests;
