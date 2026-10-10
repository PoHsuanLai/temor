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

/// What a shell pane is planned from. Every value is one the caller has already read. Build it
/// with [`ShellPaneInput::new`] and the setters; the defaults are the ones the window uses.
///
/// ```
/// use std::path::PathBuf;
/// use temor_pane::settings::{ShellChoice, TerminalSettings};
/// use temor_pane::{ShellPaneInput, shell_pane};
/// use temor_pty::{ProgramVersion, SessionLauncher};
/// use temor_vt::LineCount;
///
/// let mut settings = TerminalSettings::default();
/// settings.shell = ShellChoice::Program("/bin/sh".to_owned());
/// let input = ShellPaneInput::new(
///     PathBuf::from("/tmp/temor-runtime"),
///     PathBuf::from("/tmp"),
///     SessionLauncher::Setsid(PathBuf::from("/usr/bin/setsid")),
/// )
/// .settings(settings)
/// .version(ProgramVersion("0.0.0".to_owned()));
/// let spec = shell_pane(input, |_| true).expect("the shell is planned");
/// assert_eq!(spec.scrollback, LineCount(10_000));
/// ```
#[derive(Debug)]
#[non_exhaustive]
pub struct ShellPaneInput {
    inherited: ChildEnv,
    settings: TerminalSettings,
    passwd_shell: Option<PathBuf>,
    runtime_dir: PathBuf,
    home: PathBuf,
    version: ProgramVersion,
    pane: u64,
    grid: GridSize,
    cell: CellPx,
    palette: Palette,
    launcher: SessionLauncher,
    socket: Option<PathBuf>,
}

impl ShellPaneInput {
    /// The input with the three values that have no sensible default: where shell integration
    /// is written (private to the person), the directory the shell starts in, and how the program
    /// becomes a session leader. The rest start at their defaults: an empty inherited environment,
    /// the default settings, no passwd shell, pane 0, an 80 by 24 grid of 8 by 16 pixel cells, the
    /// default palette, and this crate's version.
    pub fn new(runtime_dir: PathBuf, home: PathBuf, launcher: SessionLauncher) -> Self {
        Self {
            inherited: ChildEnv::new(),
            settings: TerminalSettings::default(),
            passwd_shell: None,
            runtime_dir,
            home,
            version: ProgramVersion(env!("CARGO_PKG_VERSION").to_owned()),
            pane: 0,
            grid: GridSize::new(80, 24),
            cell: CellPx::new(8, 16),
            palette: Palette::default(),
            launcher,
            socket: None,
        }
    }

    /// The app's own environment. The child gets it minus what describes another terminal.
    #[must_use]
    pub fn inherited(mut self, inherited: ChildEnv) -> Self {
        self.inherited = inherited;
        self
    }

    /// The settings the pane reads (the shell and the cursor among them).
    #[must_use]
    pub fn settings(mut self, settings: TerminalSettings) -> Self {
        self.settings = settings;
        self
    }

    /// The person's login shell from the passwd entry, when the caller read it. Only the login
    /// choice uses it.
    #[must_use]
    pub fn passwd_shell(mut self, passwd_shell: Option<PathBuf>) -> Self {
        self.passwd_shell = passwd_shell;
        self
    }

    /// The app's version, as the child's `TERM_PROGRAM_VERSION` carries it.
    #[must_use]
    pub fn version(mut self, version: ProgramVersion) -> Self {
        self.version = version;
        self
    }

    /// The pane's id, so a program can address its own pane.
    #[must_use]
    pub fn pane(mut self, pane: u64) -> Self {
        self.pane = pane;
        self
    }

    /// The grid in cells, and one cell in device pixels.
    #[must_use]
    pub fn grid(mut self, grid: GridSize, cell: CellPx) -> Self {
        self.grid = grid;
        self.cell = cell;
        self
    }

    /// The pane's status socket, which the child finds in `TEMOR_SOCKET` (default: none).
    #[must_use]
    pub fn socket(mut self, socket: Option<PathBuf>) -> Self {
        self.socket = socket;
        self
    }

    /// The colours the pane starts with.
    #[must_use]
    pub fn palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }
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
        socket: input.socket,
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
