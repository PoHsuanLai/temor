//! The pure part of shell integration: which files to lay down, and how to start the shell so
//! it loads them after the person's own startup files.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use super::resolve::{Shell, ShellKind};
use super::scripts::{BASH_RC, BASH_SCRIPT, FISH_SCRIPT, ZSH_ENV, ZSH_SCRIPT};
use crate::env::ChildEnv;

/// How the shell starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShellStart {
    /// An interactive shell that reads the per-shell rc file (`~/.bashrc`, `.zshrc`).
    Interactive,
    /// An interactive login shell that also reads the profile files.
    Login,
}

/// Whether temor's integration is injected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntegrationState {
    On,
    /// `TEMOR_SHELL_INTEGRATION=0`.
    Off,
}

impl IntegrationState {
    /// What the child's environment asks for.
    pub fn of(env: &ChildEnv) -> Self {
        match env.get("TEMOR_SHELL_INTEGRATION") == Some(OsStr::new("0")) {
            true => Self::Off,
            false => Self::On,
        }
    }
}

/// One file to write under the runtime directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShimFile {
    pub path: PathBuf,
    pub contents: &'static str,
}

/// How to start a shell with temor's integration, and the files that must exist first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationPlan {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub env: ChildEnv,
    pub files: Vec<ShimFile>,
}

/// Where `XDG_DATA_DIRS` points when it is unset (the XDG base directory default).
const XDG_DATA_DIRS_DEFAULT: &str = "/usr/local/share:/usr/share";

/// The plan for starting `shell` with `env`, its shims under `runtime_dir/shell-integration`.
/// With integration off, or for a shell temor has no script for, the plan starts the shell
/// plainly and lays down nothing.
pub fn plan_integration(
    shell: &Shell,
    start: ShellStart,
    env: ChildEnv,
    runtime_dir: &Path,
) -> IntegrationPlan {
    let root = runtime_dir.join("shell-integration");
    match (IntegrationState::of(&env), shell.kind) {
        (IntegrationState::On, ShellKind::Bash) => bash(shell, start, env, &root.join("bash")),
        (IntegrationState::On, ShellKind::Zsh) => zsh(shell, start, env, &root.join("zsh")),
        (IntegrationState::On, ShellKind::Fish) => fish(shell, start, env, &root.join("fish")),
        (IntegrationState::Off, _) | (_, ShellKind::Other) => plain(shell, start, env),
    }
}

fn login_flag(start: ShellStart) -> Vec<OsString> {
    match start {
        ShellStart::Login => vec!["-l".into()],
        ShellStart::Interactive => Vec::new(),
    }
}

fn plain(shell: &Shell, start: ShellStart, env: ChildEnv) -> IntegrationPlan {
    IntegrationPlan {
        program: shell.path.clone(),
        args: login_flag(start),
        env,
        files: Vec::new(),
    }
}

/// bash ignores `--rcfile` in a login shell, so the wrapper reads the profile files itself
/// when `TEMOR_BASH_LOGIN` is set, and bash always starts as a plain interactive shell.
fn bash(shell: &Shell, start: ShellStart, mut env: ChildEnv, dir: &Path) -> IntegrationPlan {
    let rc = dir.join("temor-bashrc");
    if start == ShellStart::Login {
        env.set("TEMOR_BASH_LOGIN", "1");
    }
    IntegrationPlan {
        program: shell.path.clone(),
        args: vec!["--rcfile".into(), rc.clone().into(), "-i".into()],
        env,
        files: vec![
            ShimFile {
                path: rc,
                contents: BASH_RC.contents,
            },
            ShimFile {
                path: dir.join("temor.bash"),
                contents: BASH_SCRIPT.contents,
            },
        ],
    }
}

fn zsh(shell: &Shell, start: ShellStart, mut env: ChildEnv, dir: &Path) -> IntegrationPlan {
    if let Some(real) = env.get("ZDOTDIR").map(OsStr::to_os_string) {
        env.set("TEMOR_ZSH_ZDOTDIR", real);
    }
    env.set("ZDOTDIR", dir);
    let mut args = login_flag(start);
    args.push("-i".into());
    IntegrationPlan {
        program: shell.path.clone(),
        args,
        env,
        files: vec![
            ShimFile {
                path: dir.join(".zshenv"),
                contents: ZSH_ENV.contents,
            },
            ShimFile {
                path: dir.join("temor.zsh"),
                contents: ZSH_SCRIPT.contents,
            },
        ],
    }
}

fn fish(shell: &Shell, start: ShellStart, mut env: ChildEnv, dir: &Path) -> IntegrationPlan {
    let real = env.get("XDG_DATA_DIRS").map(OsStr::to_os_string);
    let mut dirs = OsString::from(dir);
    dirs.push(":");
    dirs.push(real.as_deref().unwrap_or(OsStr::new(XDG_DATA_DIRS_DEFAULT)));
    if let Some(real) = real {
        env.set("TEMOR_FISH_XDG_DATA_DIRS", real);
    }
    env.set("XDG_DATA_DIRS", dirs);
    let mut args = login_flag(start);
    args.push("-i".into());
    IntegrationPlan {
        program: shell.path.clone(),
        args,
        env,
        files: vec![ShimFile {
            path: dir.join("fish").join("vendor_conf.d").join("temor.fish"),
            contents: FISH_SCRIPT.contents,
        }],
    }
}
