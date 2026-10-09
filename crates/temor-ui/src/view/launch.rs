//! What the window is given at start, and how a session is opened from it: the settings, the
//! environment, the fonts, and the desktop's clipboard.

use temor_core::terminal::settings::TerminalSettings;
use temor_core::terminal::settings_value::FontChoice;
use temor_pty::SessionLauncher;
use temor_render::{FontConfig, FontFamily, FontSet, FontSize, FontSource, PixelSize, Scale};
use temor_vt::{CellPx, GridSize};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

use super::probe::Probe;
use crate::app::shell_pane;
use crate::edge::Ambient;
use crate::error::UiError;
use crate::pane::PaneEvent;
use crate::platform::default_opener;
use crate::session::{Clipboard, ClipboardFailed, Session, SessionParts};

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
    /// Where the screen's text is published after each repaint, for tests that cannot see the
    /// terminal's pixels. `None` in the app.
    pub probe: Option<Probe>,
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

/// Opens a session for the window: a shell at 80 by 24 cells, which the first layout refits.
pub fn open_session(
    launch: &Launch,
    _pixels: PixelSize,
    _scale: Scale,
) -> Result<(Session, UnboundedReceiver<PaneEvent>), UiError> {
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
    let spec = shell_pane(
        &launch.ambient,
        settings,
        1,
        grid,
        cell,
        temor_render::Palette::default(),
        launcher,
    )?;
    let (events_tx, events) = unbounded_channel();
    let session = Session::open(
        SessionParts {
            spec,
            fonts,
            keymap: settings.keymap.clone(),
            clipboard: Box::new(DesktopClipboard),
            opener: default_opener(),
        },
        events_tx,
    )?;
    Ok((session, events))
}
