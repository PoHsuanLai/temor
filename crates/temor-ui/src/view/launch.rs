//! What the window is given at start, and how a session is opened from it: the settings, the
//! environment, the fonts, and the desktop's clipboard.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use temor_client::model::vocab::ids::PaneId;
use temor_client::model::vocab::text::Cwd;
use temor_panes::pane::settings::{FontChoice, FontPoints, LinePercent, TerminalSettings};
use temor_panes::pty::SessionLauncher;
use temor_panes::render::{
    FontConfig, FontFamily, FontSet, FontSize, FontSource, LineHeight, Palette,
};
use temor_panes::shell::ChildEnv;
use temor_panes::vt::Snapshot;
use temor_panes::vt::{CellPx, GridSize};
use tokio::sync::mpsc::UnboundedSender;

use super::probe::Probe;
use super::theme::AppearanceSource;
use crate::edge::{Ambient, NoopNotifier, SystemClock};
use crate::error::UiError;
use crate::hub::Run;
use crate::pane::TaggedSink;
use crate::platform::default_opener;
use crate::session::{Clipboard, ClipboardFailed, Session, SessionKit, SessionParts};
use temor_client::model::shelf::Quoting;
use temor_client::shelf::transfer::{Bytes, Count, Limits};
use temor_panes::pane::{PaneEvent, TransferSetup};
use temor_panes::shell::ShellKind;
use temor_panes::start::{ShellStart, shell_spec};

/// Given to the session's own process as its first argument: the program becomes the session
/// leader and runs the shell (temor-pty's trampoline; the binary's `main` handles it).
pub const SESSION_LEADER_ARG: &str = "--temor-session-leader";

/// How a session's shell is started: through this program (the default, portable), or through
/// util-linux `setsid` (Linux; used by tests whose own binary cannot be the session leader).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LauncherChoice {
    ThisProgram,
    Setsid(std::path::PathBuf),
    /// A program that calls `run_session_leader` with no arguments (`temor-pty-exec`), for
    /// tests whose own binary cannot be the session leader where there is no util-linux `setsid`.
    Trampoline(std::path::PathBuf),
}

/// Everything the window starts from. Cloned into the window's root context.
#[derive(Debug, Clone)]
pub struct Launch {
    pub ambient: Ambient,
    pub settings: TerminalSettings,
    /// The keys: the desktop's conventions, the person's changes and temor's actions (see
    /// `edge::load_keymap`).
    pub keymap: chordkit::Keymap,
    pub launcher: LauncherChoice,
    /// Where the window's look (theme, accent, Space look, desktop preferences) is read from.
    pub appearance: AppearanceSource,
    /// Where the screen's text is published after each repaint, for tests that cannot see the
    /// terminal's pixels. `None` in the app.
    pub probe: Option<Probe>,
    /// Where the session file and its snapshots are kept (see `edge::resolve_state_dir`); `None`
    /// saves and restores nothing.
    pub state_dir: Option<PathBuf>,
    /// What the first window says when it opens: the settings that are not in force.
    pub first_words: FirstWords,
    /// A program the first window opens a tab for (`temor -e` when no temor was running).
    pub first_run: TakeOnce<Run>,
}

/// A value the first window takes. Every clone of the launch holds the same one, so the window
/// opened next finds it gone.
#[derive(Debug)]
pub struct TakeOnce<T>(Arc<Mutex<Option<T>>>);

impl<T> TakeOnce<T> {
    pub fn new(value: Option<T>) -> TakeOnce<T> {
        TakeOnce(Arc::new(Mutex::new(value)))
    }

    /// The value, the first time.
    pub fn take(&self) -> Option<T> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }
}

impl<T> Clone for TakeOnce<T> {
    fn clone(&self) -> Self {
        TakeOnce(Arc::clone(&self.0))
    }
}

impl<T> Default for TakeOnce<T> {
    fn default() -> Self {
        TakeOnce::new(None)
    }
}

/// Sentences for the first window to show, once. Every clone of the launch holds the same ones,
/// so the window opened next says nothing again.
#[derive(Debug, Clone, Default)]
pub struct FirstWords(Arc<Mutex<Vec<String>>>);

impl FirstWords {
    pub fn new(sentences: Vec<String>) -> FirstWords {
        FirstWords(Arc::new(Mutex::new(sentences)))
    }

    /// The sentences as one notice, the first time; `None` after, or when there were none.
    pub fn take(&self) -> Option<String> {
        let mut held = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let said = std::mem::take(&mut *held);
        (!said.is_empty()).then(|| said.join(" "))
    }
}

/// The desktop's clipboard, through the window toolkit's clipboard.
#[derive(Debug, Default)]
pub struct DesktopClipboard;

impl Clipboard for DesktopClipboard {
    fn write(&self, text: &str) -> Result<(), ClipboardFailed> {
        ds_blitz::clipboard::write_text(text).map_err(|e| ClipboardFailed(e.to_string()))
    }

    fn read(&self) -> Result<String, ClipboardFailed> {
        ds_blitz::clipboard::read_text().map_err(|e| ClipboardFailed(e.to_string()))
    }

    fn write_primary(&self, text: &str) -> Result<(), ClipboardFailed> {
        ds_blitz::clipboard::write_to(ds_blitz::clipboard::ClipboardTarget::Primary, text)
            .map_err(|e| ClipboardFailed(e.to_string()))
    }
}

pub use temor_panes::start::Program;

/// Opens a session for pane `pane`: a shell at 80 by 24 cells (the first layout refits it) in
/// `cwd` when that is a folder, else the home folder. With a `snapshot` the terminal begins as
/// that closed pane's screen and scrollback. The pane's events go to `events`, tagged with it.
pub fn open_session(
    launch: &Launch,
    palette: Palette,
    pane: PaneId,
    cwd: Option<&Cwd>,
    snapshot: Option<&Snapshot>,
    events: UnboundedSender<(PaneId, PaneEvent)>,
) -> Result<Session, UiError> {
    open_session_with(
        launch,
        palette,
        (pane, cwd, snapshot),
        events,
        (&ChildEnv::new(), None),
        None,
        None,
    )
}

/// [`open_session`] with `env` set over the surroundings' own (a Space's variables), the aliases
/// of the shell it replaces (`replay`, a script the shell reads once), the pane's status socket,
/// when it has one, named to the shell in `TEMOR_SOCKET`, and `program` run in place of the shell
/// when one is given (a host session).
pub fn open_session_with(
    launch: &Launch,
    palette: Palette,
    (pane, cwd, snapshot): (PaneId, Option<&Cwd>, Option<&Snapshot>),
    events: UnboundedSender<(PaneId, PaneEvent)>,
    (env, replay): (&ChildEnv, Option<&std::path::Path>),
    socket: Option<&std::path::Path>,
    program: Option<Program>,
) -> Result<Session, UiError> {
    let settings = &launch.settings;
    let fonts = load_fonts(&settings.font, settings.font_size, settings.line_height)?;
    let launcher = match &launch.launcher {
        LauncherChoice::ThisProgram => SessionLauncher::Trampoline {
            program: std::env::current_exe().map_err(UiError::Launcher)?,
            args: vec![SESSION_LEADER_ARG.into()],
        },
        LauncherChoice::Setsid(path) => SessionLauncher::Setsid(path.clone()),
        LauncherChoice::Trampoline(program) => SessionLauncher::Trampoline {
            program: program.clone(),
            args: Vec::new(),
        },
    };
    let ambient = &launch.ambient;
    let spec = shell_spec(ShellStart {
        runtime_dir: &ambient.runtime_dir,
        home: &ambient.home,
        env: ambient.env.clone().overlay(env),
        settings,
        pane: pane.0,
        size: first_size(),
        palette,
        launcher,
        cwd: cwd.map(Cwd::as_str),
        socket: socket.map(std::path::Path::to_path_buf),
        replay: replay.map(std::path::Path::to_path_buf),
        program,
        transfers: Some(TransferSetup {
            staging: ambient.staging_dir(pane.0),
            limits: transfer_limits(),
        }),
    })
    .map_err(UiError::from)?;
    let quoting = quoting_of(&spec.spawn.program);
    let mut session = Session::open_with(
        SessionParts {
            spec,
            fonts,
            kit: session_kit(launch),
        },
        Arc::new(TaggedSink::new(pane, events)),
        snapshot,
    )?;
    session.set_quoting(quoting);
    Ok(session)
}

/// The size a pane starts at, in cells, and a cell's in pixels: 80 by 24 of 8 by 16. The first
/// layout refits both.
pub(super) fn first_size() -> (GridSize, CellPx) {
    (GridSize::new(80, 24), CellPx::new(8, 16))
}

/// What every session is made from besides its pane: the keys, the clipboard and opener, the
/// notice settings and the downloads folder. A session on this computer's terminal and one on a
/// host's helper take the same.
pub(super) fn session_kit(launch: &Launch) -> SessionKit {
    let settings = &launch.settings;
    SessionKit {
        keymap: launch.keymap.clone(),
        clipboard: Box::new(DesktopClipboard),
        opener: default_opener(),
        notify_after: settings.notify_after,
        cursor_blink: settings.cursor_blink,
        // TODO(quire-desktop): the desktop's notification service (quire v0.2.30 has none).
        notifier: Box::new(NoopNotifier),
        clock: Box::new(SystemClock::new()),
        shelf_dir: launch.ambient.shelf_dir(),
    }
}

/// The fonts a pane draws with: `font` at `size`, found on this computer.
pub fn load_fonts(
    font: &FontChoice,
    size: FontPoints,
    line: LinePercent,
) -> Result<FontSet, UiError> {
    // The Settings window's font list is read while the first pane starts, not when it is opened.
    temor_panes::render::warm_font_catalog();
    let family = match font {
        FontChoice::SystemMonospace => FontFamily::Monospace,
        FontChoice::Named(name) => FontFamily::Named(name.clone()),
    };
    Ok(FontSet::load(
        &FontConfig {
            family,
            size: FontSize::new(size.points()),
            line_height: LineHeight::new(line.factor()),
        },
        FontSource::System,
    )?)
}

/// The caps on what a program may send through the terminal: a gigabyte a file, and as many
/// files as a folder tree reasonably has.
pub(super) fn transfer_limits() -> Limits {
    Limits::new(Bytes::new(1 << 30), Count::new(10_000), Count::new(64))
}

/// How the shell a pane runs quotes a word, for the paths a drop types into it.
fn quoting_of(shell: &std::path::Path) -> Quoting {
    match ShellKind::of(shell) {
        ShellKind::Fish => Quoting::Fish,
        ShellKind::Nushell => Quoting::Nushell,
        _ => Quoting::Posix,
    }
}
