//! Starting the person's shell (PLAN §6, temor-pty): the shell from the settings or the login
//! shell, its integration written under the runtime directory, and the complete environment.

use std::path::{Path, PathBuf};

use temor_pane::settings::ShellChoice;
use temor_pane::settings::TerminalSettings;
use temor_pty::{
    EnvContract, PaneId, ProgramVersion, SYSTEM_TERMINFO_DIRS, SessionLauncher, base_env,
    child_env, find_term, prepare_shell, terminfo_dirs,
};
use temor_shell::{ChildEnv, Shell, ShellStart, passwd_login_shell, resolve_shell};
use temor_vt::{CellPx, GridSize, LineCount};

use crate::edge::Ambient;
use crate::error::UiError;
use crate::pane::PaneSpec;

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
    let base = base_env(
        ambient
            .env
            .iter()
            .map(|(k, v)| (k.to_os_string(), v.to_os_string())),
    );
    let dirs = terminfo_dirs(&ambient.env, SYSTEM_TERMINFO_DIRS);
    let contract = EnvContract {
        term: find_term(&dirs),
        version: ProgramVersion(VERSION.to_owned()),
        pane: PaneId(pane.to_string()),
        socket: None,
    };
    let env = child_env(base, &ChildEnv::new(), &contract);
    let shell = pick_shell(settings, &env);
    let spawn = prepare_shell(
        &shell,
        ShellStart::Login,
        env,
        &ambient.runtime_dir,
        ambient.home.clone(),
        temor_pty::GridSize::cells(temor_pty::Cols(grid.cols.0), temor_pty::Rows(grid.rows.0)),
    )?;
    Ok(PaneSpec {
        spawn,
        launcher,
        grid,
        cell,
        scrollback: LineCount(settings.scrollback.lines() as usize),
        palette,
        cursor: cursor_shape(settings.cursor),
    })
}

/// The settings' shell, or the person's login shell, or the environment's `SHELL`.
fn pick_shell(settings: &TerminalSettings, env: &ChildEnv) -> Shell {
    match &settings.shell {
        ShellChoice::Program(program) => Shell::at(PathBuf::from(program)),
        ShellChoice::Login => {
            let passwd = temor_pty::current_user().and_then(|uid| {
                let text = std::fs::read_to_string("/etc/passwd").ok()?;
                passwd_login_shell(&text, uid)
            });
            resolve_shell(passwd, env, |path: &Path| path.is_file())
        }
    }
}

/// The shape a settings cursor style means for the terminal.
pub fn cursor_shape(style: temor_pane::settings::CursorStyle) -> temor_vt::CursorShape {
    use temor_pane::settings::CursorStyle;
    match style {
        CursorStyle::Block => temor_vt::CursorShape::Block,
        CursorStyle::Bar => temor_vt::CursorShape::Bar,
        CursorStyle::Underline => temor_vt::CursorShape::Underline,
    }
}
