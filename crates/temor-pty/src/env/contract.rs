//! temor's environment contract (documented in the crate docs).

use std::ffi::OsStr;
use std::path::PathBuf;

use super::model::ChildEnv;
use super::terminfo::TermName;
use ds_core::word::Word;

/// The pane this child runs in, as `TEMOR_PANE_ID` carries it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PaneId(pub String);

/// temor's version, as `TERM_PROGRAM_VERSION` carries it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProgramVersion(pub String);

/// What temor tells every child about the terminal it runs in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvContract {
    pub term: TermName,
    pub version: ProgramVersion,
    pub pane: PaneId,
    /// temor's control socket, when one is listening.
    pub socket: Option<PathBuf>,
}

/// Exact names that describe the terminal temor was started from, not the one it is.
const DROPPED: &[&str] = &[
    "TERM",
    "COLORTERM",
    "TERM_PROGRAM",
    "TERM_PROGRAM_VERSION",
    "TERM_SESSION_ID",
    "VTE_VERSION",
    "WINDOWID",
    "LINES",
    "COLUMNS",
    "TMUX",
    "TMUX_PANE",
    "STY",
];

/// Prefixes of other terminals' variable families.
const DROPPED_PREFIXES: &[&str] = &[
    "KITTY_",
    "GHOSTTY_",
    "WEZTERM_",
    "ITERM_",
    "ALACRITTY_",
    "KONSOLE_",
    "TEMOR_",
];

/// The opt-out survives the scrub, so `TEMOR_SHELL_INTEGRATION=0` in temor's own
/// environment reaches the shells it starts.
const KEPT: &[&str] = &["TEMOR_SHELL_INTEGRATION"];

/// The base environment from an injected `source` (the app passes its own environment at its
/// edge), without the variables that describe another terminal.
pub fn base_env<K, V>(source: impl IntoIterator<Item = (K, V)>) -> ChildEnv
where
    K: Into<std::ffi::OsString>,
    V: Into<std::ffi::OsString>,
{
    source
        .into_iter()
        .map(|(key, value)| (key.into(), value.into()))
        .filter(|(key, _)| kept(key))
        .collect()
}

fn kept(key: &OsStr) -> bool {
    let Some(key) = key.to_str() else {
        return true;
    };
    if KEPT.contains(&key) {
        return true;
    }
    !DROPPED.contains(&key) && !DROPPED_PREFIXES.iter().any(|p| key.starts_with(p))
}

/// The child's environment: `base`, then `TERM` and `COLORTERM`, then the Space `profile`,
/// then the identity variables, which the profile cannot override.
pub fn child_env(base: ChildEnv, profile: &ChildEnv, contract: &EnvContract) -> ChildEnv {
    let mut env = base;
    env.set("TERM", contract.term.slug());
    env.set("COLORTERM", "truecolor");
    let mut env = env.overlay(profile);
    env.set("TERM_PROGRAM", "temor");
    env.set("TERM_PROGRAM_VERSION", &contract.version.0);
    env.set("TEMOR_PANE_ID", &contract.pane.0);
    match &contract.socket {
        Some(path) => env.set("TEMOR_SOCKET", path),
        None => {
            env.remove("TEMOR_SOCKET");
        }
    }
    env
}
