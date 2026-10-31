//! A shell pane's spec from the window's surroundings (`temor_panes::start` plans it), for the
//! tests that start a pane with no window.

use std::path::PathBuf;

use temor_panes::pane::PaneSpec;
use temor_panes::pane::settings::TerminalSettings;
use temor_panes::pty::SessionLauncher;
use temor_panes::start::{ShellStart, shell_spec};
use temor_panes::vt::{CellPx, GridSize};

use crate::edge::Ambient;
use crate::error::UiError;

/// Builds the pane spec for a new shell of `grid` cells of `cell` pixels.
pub fn shell_pane(
    ambient: &Ambient,
    settings: &TerminalSettings,
    pane: u64,
    grid: GridSize,
    cell: CellPx,
    palette: temor_panes::render::Palette,
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
    size: (GridSize, CellPx),
    palette: temor_panes::render::Palette,
    launcher: SessionLauncher,
    (socket, replay): (Option<PathBuf>, Option<PathBuf>),
) -> Result<PaneSpec, UiError> {
    Ok(shell_spec(ShellStart {
        runtime_dir: &ambient.runtime_dir,
        home: &ambient.home,
        env: ambient.env.clone(),
        settings,
        pane,
        size,
        palette,
        launcher,
        cwd: None,
        socket,
        replay,
        program: None,
        transfers: None,
    })?)
}
