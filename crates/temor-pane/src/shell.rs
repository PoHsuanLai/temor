//! The shell pane: the person's shell, its integration and the complete environment, planned from
//! values the caller has read (the environment, the login shell, the runtime folder). Pure: no
//! environment, passwd or filesystem reads happen here; the caller reads them at its edge.

use std::path::{Path, PathBuf};

use temor_pty::{
    EnvContract, PaneId, ProgramVersion, SYSTEM_TERMINFO_DIRS, SessionLauncher, base_env,
    child_env, find_term, prepare_shell, terminfo_dirs,
};
use temor_render::Palette;
use temor_shell::{ChildEnv, Shell, ShellStart, resolve_shell};
use temor_vt::{CellPx, CursorShape, GridSize, LineCount};

use crate::PaneSpec;
use crate::error::PaneError;
use crate::settings::{CursorStyle, ShellChoice, TerminalSettings};

/// What a shell pane is planned from. Every field is a value the caller has already read.
///
/// ```
/// use std::path::PathBuf;
/// use temor_pane::settings::{ShellChoice, TerminalSettings};
/// use temor_pane::{ShellPaneInput, shell_pane};
/// use temor_pty::{ProgramVersion, SessionLauncher};
/// use temor_render::Palette;
/// use temor_shell::ChildEnv;
/// use temor_vt::{CellPx, GridSize, LineCount};
///
/// let mut settings = TerminalSettings::default();
/// settings.shell = ShellChoice::Program("/bin/sh".to_owned());
/// let input = ShellPaneInput {
///     inherited: ChildEnv::new(),
///     settings,
///     passwd_shell: None,
///     runtime_dir: PathBuf::from("/tmp/temor-runtime"),
///     home: PathBuf::from("/tmp"),
///     version: ProgramVersion("0.0.0".to_owned()),
///     pane: 1,
///     grid: GridSize::new(80, 24),
///     cell: CellPx::new(8, 16),
///     palette: Palette::default(),
///     launcher: SessionLauncher::Setsid(PathBuf::from("/usr/bin/setsid")),
/// };
/// let spec = shell_pane(input, |_| true).expect("the shell is planned");
/// assert_eq!(spec.scrollback, LineCount(10_000));
/// ```
#[derive(Debug)]
pub struct ShellPaneInput {
    /// The app's own environment. The child gets it minus what describes another terminal.
    pub inherited: ChildEnv,
    /// The settings the pane reads (the shell and the cursor among them).
    pub settings: TerminalSettings,
    /// The person's login shell from the passwd entry, when the caller read it. Only the login
    /// choice uses it.
    pub passwd_shell: Option<PathBuf>,
    /// Where shell integration files are written. It must be private to the person.
    pub runtime_dir: PathBuf,
    /// The directory the shell starts in.
    pub home: PathBuf,
    /// The app's version, as the child's `TERM_PROGRAM_VERSION` carries it.
    pub version: ProgramVersion,
    /// The pane's id, so a program can address its own pane.
    pub pane: u64,
    /// The grid in cells, and one cell in device pixels.
    pub grid: GridSize,
    pub cell: CellPx,
    pub palette: Palette,
    /// How the program becomes a session leader.
    pub launcher: SessionLauncher,
}

/// Plans the spec for a new shell pane. `is_file` says whether a candidate program exists; it is
/// the only question asked of the filesystem, and the caller answers it.
pub fn shell_pane(
    input: ShellPaneInput,
    is_file: impl Fn(&Path) -> bool,
) -> Result<PaneSpec, PaneError> {
    let base = base_env(
        input
            .inherited
            .iter()
            .map(|(key, value)| (key.to_os_string(), value.to_os_string())),
    );
    let dirs = terminfo_dirs(&input.inherited, SYSTEM_TERMINFO_DIRS);
    let contract = EnvContract {
        term: find_term(&dirs),
        version: input.version,
        pane: PaneId(input.pane.to_string()),
        socket: None,
    };
    let env = child_env(base, &ChildEnv::new(), &contract);
    let shell = pick_shell(&input.settings, &env, input.passwd_shell, is_file);
    let spawn = prepare_shell(
        &shell,
        ShellStart::Login,
        env,
        &input.runtime_dir,
        input.home,
        temor_pty::GridSize::cells(
            temor_pty::Cols(input.grid.cols.0),
            temor_pty::Rows(input.grid.rows.0),
        ),
    )?;
    Ok(PaneSpec {
        spawn,
        launcher: input.launcher,
        grid: input.grid,
        cell: input.cell,
        scrollback: LineCount(input.settings.scrollback.lines() as usize),
        palette: input.palette,
        cursor: cursor_shape(input.settings.cursor),
    })
}

/// The settings' shell, or the person's login shell, or the environment's `SHELL`.
fn pick_shell(
    settings: &TerminalSettings,
    env: &ChildEnv,
    passwd: Option<PathBuf>,
    is_file: impl Fn(&Path) -> bool,
) -> Shell {
    match &settings.shell {
        ShellChoice::Program(program) => Shell::at(PathBuf::from(program)),
        ShellChoice::Login => resolve_shell(passwd, env, is_file),
    }
}

/// The shape a settings cursor style means for the terminal.
pub fn cursor_shape(style: CursorStyle) -> CursorShape {
    match style {
        CursorStyle::Block => CursorShape::Block,
        CursorStyle::Bar => CursorShape::Bar,
        CursorStyle::Underline => CursorShape::Underline,
    }
}
