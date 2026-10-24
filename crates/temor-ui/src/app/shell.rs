//! Starting the person's shell at the app's edge: the environment, the login shell and the runtime
//! folder are read here, and temor-pane plans the spec from them.

use std::path::{Path, PathBuf};

use temor_pane::PaneSpec;
use temor_pane::settings::{ShellChoice, TerminalSettings};
use temor_pty::{ProgramVersion, SessionLauncher, login_shell};
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
    shell_pane_with_socket(
        ambient,
        settings,
        pane,
        (grid, cell),
        palette,
        launcher,
        (None, None),
    )
}

/// Like [`shell_pane`], with the pane's status socket named to the shell in `TEMOR_SOCKET` and the
/// aliases of a shell it replaces in `TEMOR_REPLAY`. Every shell gets a new report key.
pub fn shell_pane_with_socket(
    ambient: &Ambient,
    settings: &TerminalSettings,
    pane: u64,
    (grid, cell): (GridSize, CellPx),
    palette: temor_render::Palette,
    launcher: SessionLauncher,
    (socket, replay): (Option<PathBuf>, Option<PathBuf>),
) -> Result<PaneSpec, UiError> {
    // The passwd file is read only when the login shell is asked for.
    let passwd_shell = match settings.shell {
        ShellChoice::Login => login_shell(),
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
    .socket(socket)
    .replay(replay)
    .report_key(Some(crate::edge::replay::fresh_key().as_str().to_owned()))
    .grid(grid, cell)
    .palette(palette);
    Ok(temor_pane::shell_pane(input, |path: &Path| path.is_file())?)
}
