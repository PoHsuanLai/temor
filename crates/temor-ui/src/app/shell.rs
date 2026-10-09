//! Starting the person's shell at the app's edge: the environment, the login shell and the runtime
//! folder are read here, and temor-pane plans the spec from them.

use std::path::{Path, PathBuf};

use temor_pane::PaneSpec;
use temor_pane::settings::{ShellChoice, TerminalSettings};
use temor_pty::{ProgramVersion, SessionLauncher, current_user};
use temor_shell::passwd_login_shell;
use temor_vt::{CellPx, GridSize};

use crate::edge::Ambient;
use crate::error::UiError;

/// The version temor tells children (`TERM_PROGRAM_VERSION`).
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Builds the pane spec for a new shell of `grid` cells of `cell` pixels.
pub fn shell_pane(
    ambient: &Ambient,
    settings: &TerminalSettings,
    pane: u64,
    grid: GridSize,
    cell: CellPx,
    palette: temor_render::Palette,
    launcher: SessionLauncher,
) -> Result<PaneSpec, UiError> {
    // The passwd file is read only when the login shell is asked for.
    let passwd_shell = match settings.shell {
        ShellChoice::Login => login_shell_from_passwd(),
        ShellChoice::Program(_) => None,
    };
    let input = temor_pane::ShellPaneInput::new(
        ambient.runtime_dir.clone(),
        ambient.home.clone(),
        launcher,
    )
    .inherited(ambient.env.clone())
    .settings(settings.clone())
    .passwd_shell(passwd_shell)
    .version(ProgramVersion(VERSION.to_owned()))
    .pane(pane)
    .grid(grid, cell)
    .palette(palette);
    Ok(temor_pane::shell_pane(input, |path: &Path| path.is_file())?)
}

/// The person's login shell from `/etc/passwd`, when the passwd lookup finds it.
fn login_shell_from_passwd() -> Option<PathBuf> {
    let uid = current_user()?;
    let text = std::fs::read_to_string("/etc/passwd").ok()?;
    passwd_login_shell(&text, uid)
}
