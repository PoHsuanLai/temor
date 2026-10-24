//! What a pane's shell holds that a new shell would need to look the same: its folder, exported
//! variables and aliases. Built from the shell's own reports, one step at a time, so the same
//! reports in the same order give the same state; a change is said as a [`ShellDelta`] and the
//! whole state can be turned into what a fresh shell must be told ([`ShellState::replay`]).
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

/// Words that make a name a secret wherever they stand in it.
const SECRET_WORDS: &[&str] = &[
    "TOKEN",
    "SECRET",
    "PASSWORD",
    "PASSWD",
    "PASSPHRASE",
    "CREDENTIAL",
    "PRIVATE",
    "APIKEY",
    "API_KEY",
    "ACCESS_KEY",
];

/// Parts of a name (between underscores) that make it a secret: `DB_PASS`, `GITLAB_PAT`,
/// `SENTRY_DSN`, `NPM_AUTH`. As parts only, so `MY_PATH` and `AUTHOR` are not.
const SECRET_PARTS: &[&str] = &["PASS", "PWD", "PAT", "DSN", "AUTH"];

/// Whether a variable's name says its value is a secret.
pub fn is_secret(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    SECRET_WORDS.iter().any(|word| upper.contains(word))
        || upper.split('_').any(|part| SECRET_PARTS.contains(&part))
        || upper.ends_with("_KEY")
}

/// Starts of well-known tokens (GitHub, GitLab, OpenAI and others, Slack); a word that starts
/// with one and goes on for 16 or more token characters is taken for a token.
const TOKEN_PREFIXES: &[&str] = &[
    "ghp_",
    "gho_",
    "ghs_",
    "ghu_",
    "github_pat_",
    "glpat-",
    "sk-",
    "xoxb-",
    "xoxp-",
];

/// Starts of AWS access key ids, which go on with 16 capitals or digits.
const AWS_PREFIXES: &[&str] = &["AKIA", "ASIA"];

fn token_like(word: &str) -> bool {
    let tail_is = |prefix: &str, ok: fn(u8) -> bool| {
        word.strip_prefix(prefix)
            .is_some_and(|rest| rest.len() >= 16 && rest.bytes().all(ok))
    };
    TOKEN_PREFIXES
        .iter()
        .any(|p| tail_is(p, |b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'))
        || AWS_PREFIXES
            .iter()
            .any(|p| tail_is(p, |b| b.is_ascii_uppercase() || b.is_ascii_digit()))
}

/// Whether `text` has a URL with a password in it (`scheme://user:password@host`).
fn url_with_password(text: &str) -> bool {
    text.match_indices("://").any(|(at, _)| {
        let rest = &text[at + 3..];
        let authority = rest
            .split(|c: char| c == '/' || c == '?' || c == '#' || c.is_whitespace())
            .next()
            .unwrap_or("");
        authority
            .rsplit_once('@')
            .is_some_and(|(user, _)| user.contains(':'))
    })
}

/// Whether a value looks like a secret whatever its name: a URL with a password in it, or a
/// word that looks like a well-known token.
pub fn looks_secret(value: &str) -> bool {
    url_with_password(value)
        || value
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
            .any(token_like)
}

/// Whether a line of shell (an alias's value, a command) holds a secret: it sets a variable
/// whose name is a secret's (`export DB_PASSWORD=...`), or something in it looks like one.
pub fn holds_secret(line: &str) -> bool {
    looks_secret(line)
        || line
            .split(|c: char| c.is_whitespace() || ";&|()`".contains(c))
            .filter_map(|word| word.split_once('='))
            .any(|(name, _)| is_variable_name(name) && is_secret(name))
}

fn is_variable_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
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
    match is_secret(name) || looks_secret(value) {
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
                .filter(|(name, _)| !never_carried(name))
                .map(|(name, value)| (name.clone(), entry_of(name, value)))
                .collect();
            let aliases: BTreeMap<String, String> = report
                .aliases
                .into_iter()
                .filter(|(_, value)| !holds_secret(value))
                .collect();
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
