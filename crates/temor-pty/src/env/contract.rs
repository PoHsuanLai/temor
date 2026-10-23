//! temor's environment contract (documented in the crate docs).

use std::ffi::OsStr;
use std::path::PathBuf;

use super::terminfo::TermName;
use temor_shell::ChildEnv;

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
    "DESKTOP_STARTUP_ID",
    "XDG_ACTIVATION_TOKEN",
    "OLDPWD",
    "LC_TERMINAL",
    "LC_TERMINAL_VERSION",
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

/// The person's choices survive the scrub, so `TEMOR_SHELL_INTEGRATION=0` and `TEMOR_PROMPT`
/// in temor's own environment reach the shells it starts. `TEMOR_REPLAY` names the aliases a
/// replaced shell leaves for the new one; the shell deletes the file and the variable at its
/// first prompt.
const KEPT: &[&str] = &[
    "TEMOR_SHELL_INTEGRATION",
    "TEMOR_PROMPT",
    "TEMOR_PAGER",
    "TEMOR_REPLAY",
];

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

/// temor's own variables, which only temor sets (the opt-out is the person's to set).
fn temor_internal(key: &OsStr) -> bool {
    key.to_str()
        .is_some_and(|key| key.starts_with("TEMOR_") && !KEPT.contains(&key))
}

/// The child's environment: `base`, then `TERM` and `COLORTERM`, then the Space `profile`,
/// then the identity variables, which the profile cannot override. `TEMOR_*` variables in the
/// profile are dropped, except `TEMOR_SHELL_INTEGRATION`.
pub fn child_env(base: ChildEnv, profile: &ChildEnv, contract: &EnvContract) -> ChildEnv {
    let mut env = base;
    env.set("TERM", contract.term.to_string());
    env.set("COLORTERM", "truecolor");
    let profile: ChildEnv = profile
        .iter()
        .filter(|(key, _)| !temor_internal(key))
        .collect();
    let mut env = env.overlay(&profile);
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
