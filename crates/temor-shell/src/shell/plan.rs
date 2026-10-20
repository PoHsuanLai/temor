//! The pure part of shell integration: which files to lay down, and how to start the shell so
//! it loads them after the person's own startup files.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use super::resolve::{Shell, ShellKind};
use super::scripts::{
    BASH_RC, BASH_SCRIPT, FISH_SCRIPT, NU_SCRIPT, ShellScript, ZSH_ENV, ZSH_SCRIPT,
};
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

/// Where `XDG_DATA_DIRS` points when it is unset (the XDG base directory default). The SSH
/// bootstrap uses the same value.
pub const XDG_DATA_DIRS_DEFAULT: &str = "/usr/local/share:/usr/share";

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
        (IntegrationState::On, ShellKind::Bash) => bash(shell, start, env, &root),
        (IntegrationState::On, ShellKind::Zsh) => zsh(shell, start, env, &root),
        (IntegrationState::On, ShellKind::Fish) => fish(shell, start, env, &root),
        (IntegrationState::On, ShellKind::Nushell) => nushell(shell, start, env, &root),
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

/// The file to write for `script` under the shim root.
fn shim(root: &Path, script: ShellScript) -> ShimFile {
    ShimFile {
        path: root.join(script.folder).join(script.path),
        contents: script.contents,
    }
}

/// Puts `dir` first in `XDG_DATA_DIRS` (the default when unset) and keeps the person's own value
/// in `saved_var`, which the shell's script puts back once it has been read.
fn prepend_data_dir(env: &mut ChildEnv, dir: &Path, saved_var: &str) {
    let real = env.get("XDG_DATA_DIRS").map(OsStr::to_os_string);
    let mut dirs = OsString::from(dir);
    dirs.push(":");
    dirs.push(real.as_deref().unwrap_or(OsStr::new(XDG_DATA_DIRS_DEFAULT)));
    if let Some(real) = real {
        env.set(saved_var, real);
    }
    env.set("XDG_DATA_DIRS", dirs);
}

/// bash ignores `--rcfile` in a login shell, so the wrapper reads the profile files itself
/// when `TEMOR_BASH_LOGIN` is set, and bash always starts as a plain interactive shell.
fn bash(shell: &Shell, start: ShellStart, mut env: ChildEnv, root: &Path) -> IntegrationPlan {
    let rc = shim(root, BASH_RC);
    if start == ShellStart::Login {
        env.set("TEMOR_BASH_LOGIN", "1");
    }
    IntegrationPlan {
        program: shell.path.clone(),
        args: vec!["--rcfile".into(), rc.path.clone().into(), "-i".into()],
        env,
        files: vec![rc, shim(root, BASH_SCRIPT)],
    }
}

fn zsh(shell: &Shell, start: ShellStart, mut env: ChildEnv, root: &Path) -> IntegrationPlan {
    if let Some(real) = env.get("ZDOTDIR").map(OsStr::to_os_string) {
        env.set("TEMOR_ZSH_ZDOTDIR", real);
    }
    env.set("ZDOTDIR", root.join(ZSH_ENV.folder));
    let mut args = login_flag(start);
    args.push("-i".into());
    IntegrationPlan {
        program: shell.path.clone(),
        args,
        env,
        files: vec![shim(root, ZSH_ENV), shim(root, ZSH_SCRIPT)],
    }
}

fn fish(shell: &Shell, start: ShellStart, mut env: ChildEnv, root: &Path) -> IntegrationPlan {
    prepend_data_dir(
        &mut env,
        &root.join(FISH_SCRIPT.folder),
        "TEMOR_FISH_XDG_DATA_DIRS",
    );
    let mut args = login_flag(start);
    args.push("-i".into());
    IntegrationPlan {
        program: shell.path.clone(),
        args,
        env,
        files: vec![shim(root, FISH_SCRIPT)],
    }
}

/// nushell finds vendor autoload files under `XDG_DATA_DIRS/nushell/vendor/autoload`. The
/// integration file there switches on nushell's own marks; the entry this adds is taken back out
/// by that file, as fish's is.
fn nushell(shell: &Shell, start: ShellStart, mut env: ChildEnv, root: &Path) -> IntegrationPlan {
    prepend_data_dir(
        &mut env,
        &root.join(NU_SCRIPT.folder),
        "TEMOR_NU_XDG_DATA_DIRS",
    );
    IntegrationPlan {
        program: shell.path.clone(),
        args: login_flag(start),
        env,
        files: vec![shim(root, NU_SCRIPT)],
    }
}
