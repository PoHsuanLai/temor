//! The pure part of shell integration: which files to lay down, and how to start the shell so
//! it loads them after the person's own startup files.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use super::adapter::adapter_for;
use super::resolve::Shell;
use super::scripts::ShellScript;
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
    match IntegrationState::of(&env) {
        IntegrationState::On => adapter_for(shell.kind).plan(shell, start, env, &root),
        IntegrationState::Off => plain(shell, start, env),
    }
}

pub(super) fn login_flag(start: ShellStart) -> Vec<OsString> {
    match start {
        ShellStart::Login => vec!["-l".into()],
        ShellStart::Interactive => Vec::new(),
    }
}

pub(super) fn plain(shell: &Shell, start: ShellStart, env: ChildEnv) -> IntegrationPlan {
    IntegrationPlan {
        program: shell.path.clone(),
        args: login_flag(start),
        env,
        files: Vec::new(),
    }
}

/// The file to write for `script` under the shim root.
pub(super) fn shim(root: &Path, script: ShellScript) -> ShimFile {
    ShimFile {
        path: root.join(script.folder).join(script.path),
        contents: script.contents,
    }
}

/// Puts `dir` first in `XDG_DATA_DIRS` (the default when unset) and keeps the person's own value
/// in `saved_var`, which the shell's script puts back once it has been read.
pub(super) fn prepend_data_dir(env: &mut ChildEnv, dir: &Path, saved_var: &str) {
    let real = env.get("XDG_DATA_DIRS").map(OsStr::to_os_string);
    let mut dirs = OsString::from(dir);
    dirs.push(":");
    dirs.push(real.as_deref().unwrap_or(OsStr::new(XDG_DATA_DIRS_DEFAULT)));
    if let Some(real) = real {
        env.set(saved_var, real);
    }
    env.set("XDG_DATA_DIRS", dirs);
}
