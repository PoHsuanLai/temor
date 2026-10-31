//! What a pane on this computer runs, planned from what the window read at start (PLAN §2b):
//! its private runtime folder made, the folder it starts in checked, the login shell read when
//! asked for, a new report key, and the program a host session runs in place of the shell.
//! The pane itself is opened by the window, with how it is drawn.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use temor_pane::settings::{PagerChoice, ShellChoice, TerminalSettings};
use temor_pane::{ByteTap, PaneError, PaneSpec, ShellPaneInput, TransferSetup, shell_pane};
use temor_pty::{ProgramVersion, SessionLauncher, login_shell};
use temor_render::Palette;
use temor_shell::ChildEnv;
use temor_vt::{CellPx, GridSize};
use thiserror::Error;

/// The version temor tells children (`TERM_PROGRAM_VERSION`).
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A program to run in a pane in place of the person's shell (the `ssh` of a host session).
#[derive(Debug, Clone)]
pub struct Program {
    pub program: PathBuf,
    pub args: Vec<std::ffi::OsString>,
    /// Hears what the program prints, before the terminal parses it.
    pub tap: Option<Arc<dyn ByteTap>>,
}

/// Where a pane's shell starts from.
#[derive(Debug, Clone)]
pub struct ShellStart<'a> {
    /// Where the shell integration's files are written, private to the person.
    pub runtime_dir: &'a Path,
    /// The folder the shell starts in when `cwd` is not one.
    pub home: &'a Path,
    /// The environment it starts from: the process's, with a Space's variables over it.
    pub env: ChildEnv,
    pub settings: &'a TerminalSettings,
    pub pane: u64,
    pub size: (GridSize, CellPx),
    pub palette: Palette,
    pub launcher: SessionLauncher,
    /// The folder the pane was in, if any; it starts there when that is still a folder.
    pub cwd: Option<&'a str>,
    /// The pane's status socket, named to the shell in `TEMOR_SOCKET`.
    pub socket: Option<PathBuf>,
    /// The aliases of a shell it replaces, a script the shell reads once (`TEMOR_REPLAY`).
    pub replay: Option<PathBuf>,
    /// Run in place of the shell, in the shell's environment and size.
    pub program: Option<Program>,
    pub transfers: Option<TransferSetup>,
}

/// Why a pane's program could not be planned.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StartError {
    /// The private runtime folder could not be made.
    #[error("the runtime folder could not be made private: {0}")]
    RuntimeDir(#[source] std::io::Error),
    #[error(transparent)]
    Pane(#[from] PaneError),
}

/// The spec of the pane `start` describes.
pub fn shell_spec(start: ShellStart<'_>) -> Result<PaneSpec, StartError> {
    temor_client::keep::private::ensure_private_dir(start.runtime_dir)
        .map_err(StartError::RuntimeDir)?;
    let mut env = start.env;
    if start.settings.pager == PagerChoice::System && env.get("TEMOR_PAGER").is_none() {
        env.set("TEMOR_PAGER", "system");
    }
    let home = start
        .cwd
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute() && dir.is_dir())
        .unwrap_or_else(|| start.home.to_path_buf());
    // The passwd file is read only when the login shell is asked for.
    let passwd_shell = match start.settings.shell {
        ShellChoice::Login => login_shell(),
        ShellChoice::Program(_) => None,
    };
    let (grid, cell) = start.size;
    let input = ShellPaneInput::new(start.runtime_dir.to_path_buf(), home, start.launcher)
        .inherited(env)
        .settings(start.settings.clone())
        .passwd_shell(passwd_shell)
        .version(ProgramVersion(VERSION.to_owned()))
        .pane(start.pane)
        .socket(start.socket)
        .replay(start.replay)
        .report_key(Some(
            temor_client::keep::replay::fresh_key().as_str().to_owned(),
        ))
        .grid(grid, cell)
        .palette(start.palette);
    let mut spec = shell_pane(input, |path: &Path| path.is_file())?;
    spec.transfers = start.transfers;
    if let Some(run) = start.program {
        // The pane's environment and size stay; only what runs in it changes.
        spec.spawn.program = run.program;
        spec.spawn.args = run.args;
        spec.tap = run.tap;
    }
    Ok(spec)
}
