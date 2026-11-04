//! Which shell to start: the passwd login shell, then `$SHELL`, then `/bin/sh`.

use std::path::{Path, PathBuf};

use super::scripts::{BASH_SCRIPT, FISH_SCRIPT, NU_SCRIPT, ShellScript, ZSH_SCRIPT};
use crate::env::ChildEnv;

/// The shells temor has integration for, and everything else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ShellKind {
    Bash,
    Zsh,
    Fish,
    Nushell,
    Other,
}

impl ShellKind {
    /// Each shell and the name its program has (nushell also answers to `nushell`).
    const NAMED: [(ShellKind, &'static str); 4] = [
        (Self::Bash, "bash"),
        (Self::Zsh, "zsh"),
        (Self::Fish, "fish"),
        (Self::Nushell, "nu"),
    ];

    /// The kind named by a shell's path (`/usr/bin/zsh`, `-bash`, `fish`). The file name is the
    /// shell's name exactly: `zsh-5.9` is not zsh.
    pub fn of(path: &Path) -> Self {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .trim_start_matches('-');
        match name {
            "nushell" => Self::Nushell,
            _ => Self::NAMED
                .into_iter()
                .find(|(_, stem)| *stem == name)
                .map_or(Self::Other, |(kind, _)| kind),
        }
    }

    /// The kind a running process's name stands for (`zsh`, `-zsh`, `/bin/bash`, `zsh-5.9`). A
    /// name that starts with a shell's name counts, since a process list shows what the program
    /// calls itself (`comm`), which can carry a version; for a path to choose a shell, use
    /// [`ShellKind::of`].
    pub fn from_process_name(name: &str) -> Self {
        let name = name.trim().rsplit('/').next().unwrap_or_default();
        let name = name.trim_start_matches('-');
        Self::NAMED
            .into_iter()
            .find(|(_, stem)| name.starts_with(stem))
            .map_or(Self::Other, |(kind, _)| kind)
    }

    /// The integration script this shell sources, or `None` for a shell temor has none for.
    pub fn script(self) -> Option<ShellScript> {
        match self {
            Self::Bash => Some(BASH_SCRIPT),
            Self::Zsh => Some(ZSH_SCRIPT),
            Self::Fish => Some(FISH_SCRIPT),
            Self::Nushell => Some(NU_SCRIPT),
            Self::Other => None,
        }
    }
}

/// A shell program and its kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shell {
    pub path: PathBuf,
    pub kind: ShellKind,
}

impl Shell {
    /// The shell at `path`, its kind read from the file name.
    pub fn at(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let kind = ShellKind::of(&path);
        Self { path, kind }
    }
}

/// The fallback every Unix has.
const FALLBACK: &str = "/bin/sh";

/// The first usable of: `passwd_shell` (from the caller's passwd lookup), `$SHELL` in `env`,
/// then `/bin/sh`. A candidate is usable when it is an absolute path, is not a no-login
/// placeholder (`nologin`, `false`), and `is_executable` says it can be run.
pub fn resolve_shell(
    passwd_shell: Option<PathBuf>,
    env: &ChildEnv,
    is_executable: impl Fn(&Path) -> bool,
) -> Shell {
    let from_env = env.get("SHELL").map(PathBuf::from);
    [passwd_shell, from_env]
        .into_iter()
        .flatten()
        .find(|path| usable(path) && is_executable(path))
        .map_or_else(|| Shell::at(FALLBACK), Shell::at)
}

fn usable(path: &Path) -> bool {
    let placeholder = matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("nologin" | "false")
    );
    path.is_absolute() && !placeholder
}

/// A numeric user id, as passwd lists it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(pub u32);
