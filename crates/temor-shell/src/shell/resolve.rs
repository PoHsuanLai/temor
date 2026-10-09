//! Which shell to start: the passwd login shell, then `$SHELL`, then `/bin/sh`.

use std::path::{Path, PathBuf};

use crate::env::ChildEnv;

/// The shells temor has integration for, and everything else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShellKind {
    Bash,
    Zsh,
    Fish,
    Nushell,
    Other,
}

impl ShellKind {
    /// The kind named by a shell's path (`/usr/bin/zsh`, `-bash`, `fish`).
    pub fn of(path: &Path) -> Self {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .trim_start_matches('-');
        match name {
            "bash" => Self::Bash,
            "zsh" => Self::Zsh,
            "fish" => Self::Fish,
            "nu" | "nushell" => Self::Nushell,
            _ => Self::Other,
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

/// The login shell of user `uid` in passwd-format `text` (`name:pw:uid:gid:gecos:home:shell`),
/// for callers that read `/etc/passwd` or `getent passwd` output at their edge.
pub fn passwd_login_shell(text: &str, uid: UserId) -> Option<PathBuf> {
    let uid = uid.0.to_string();
    text.lines()
        .map(|line| line.split(':').collect::<Vec<_>>())
        .find(|fields| fields.len() == 7 && fields[2] == uid)
        .map(|fields| fields[6])
        .filter(|shell| !shell.is_empty())
        .map(PathBuf::from)
}
