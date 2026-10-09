//! What the window is given at start, and how a session is opened from it: the settings, the
//! environment, the fonts, and the desktop's clipboard.

use std::path::PathBuf;
use std::sync::Arc;

use temor_core::vocab::ids::PaneId;
use temor_core::vocab::text::Cwd;
use temor_pane::settings::FontChoice;
use temor_pane::settings::TerminalSettings;
use temor_pty::SessionLauncher;
use temor_render::{FontConfig, FontFamily, FontSet, FontSize, FontSource, Palette};
use temor_vt::{CellPx, GridSize};
use temor_vt::Snapshot;
use tokio::sync::mpsc::UnboundedSender;

use super::probe::Probe;
use super::theme::AppearanceSource;
use crate::app::shell_pane;
use crate::edge::{Ambient, NoopNotifier, SystemClock};
use crate::error::UiError;
use crate::pane::TaggedSink;
use crate::platform::{default_opener, ensure_private_dir};
use crate::session::{Clipboard, ClipboardFailed, Session, SessionParts};
use temor_pane::PaneEvent;

/// Given to the session's own process as its first argument: the program becomes the session
/// leader and runs the shell (temor-pty's trampoline; the binary's `main` handles it).
pub const SESSION_LEADER_ARG: &str = "--temor-session-leader";

/// How a session's shell is started: through this program (the default, portable), or through
/// util-linux `setsid` (Linux; used by tests whose own binary cannot be the session leader).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LauncherChoice {
    ThisProgram,
    Setsid(std::path::PathBuf),
}

/// Everything the window starts from. Cloned into the window's root context.
#[derive(Debug, Clone)]
pub struct Launch {
    pub ambient: Ambient,
    pub settings: TerminalSettings,
    pub launcher: LauncherChoice,
    /// Where the window's look (theme, accent, Space look, desktop preferences) is read from.
    pub appearance: AppearanceSource,
    /// Where the screen's text is published after each repaint, for tests that cannot see the
    /// terminal's pixels. `None` in the app.
    pub probe: Option<Probe>,
    /// Where the session file and its snapshots are kept (see `edge::resolve_state_dir`); `None`
    /// saves and restores nothing.
    pub state_dir: Option<PathBuf>,
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
}

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
    ensure_private_dir(&launch.ambient.runtime_dir).map_err(UiError::RuntimeDir)?;
    let settings = &launch.settings;
    let family = match &settings.font {
        FontChoice::SystemMonospace => FontFamily::Monospace,
        FontChoice::Named(name) => FontFamily::Named(name.clone()),
    };
    let fonts = FontSet::load(
        &FontConfig {
            family,
            size: FontSize::new(settings.font_size.points()),
        },
        FontSource::System,
    )?;
    let launcher = match &launch.launcher {
        LauncherChoice::ThisProgram => SessionLauncher::Trampoline {
            program: std::env::current_exe().map_err(UiError::Launcher)?,
            args: vec![SESSION_LEADER_ARG.into()],
        },
        LauncherChoice::Setsid(path) => SessionLauncher::Setsid(path.clone()),
    };
    let grid = GridSize::new(80, 24);
    let cell = CellPx::new(8, 16);
    // The shell starts in the pane's folder when it is still there.
    let mut ambient = launch.ambient.clone();
    if let Some(dir) = cwd
        .map(|c| PathBuf::from(c.as_str()))
        .filter(|p| p.is_absolute() && p.is_dir())
    {
        ambient.home = dir;
    }
    let spec = shell_pane(&ambient, settings, pane.0, grid, cell, palette, launcher)?;
    Session::open_with(
        SessionParts {
            spec,
            fonts,
            keymap: settings.keymap.clone(),
            clipboard: Box::new(DesktopClipboard),
            opener: default_opener(),
            notify_after: settings.notify_after,
            // TODO(quire-desktop): the desktop's notification service (quire v0.2.30 has none).
            notifier: Box::new(NoopNotifier),
            clock: Box::new(SystemClock::new()),
            shelf_dir: launch.ambient.shelf_dir(),
        },
        Arc::new(TaggedSink::new(pane, events)),
        snapshot,
    )
}
