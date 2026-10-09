//! The session's state and the types it reports: the struct, its parts, the notices it
//! gives the window, and the drag and query records. Fields are visible to the session's
//! children only.

use std::path::PathBuf;

use ds_core::time::stamp::Stamp;
use temor_core::notify::{Focus, Limiter};
use temor_core::vocab::ids::BlockId;
use temor_pane::keys::Keymap;
use temor_pane::settings::NotifyAfter;
use temor_pane::text::Finder;
use temor_pane::text::Selection;
use temor_pty::ChildExit;
use temor_render::{FontSet, GridFit, PixelSize, Scale};
use temor_vt::Progress;

use super::blocks::BlockLog;
use super::clip::PendingPaste;
use super::clipboard::Clipboard;
use crate::edge::{Clock, Notifier};
use crate::input::ClickState;
use crate::paint::Painter;
use crate::pane::{Hold, PaneHandle, PaneSpec};
use crate::platform::Opener;

/// What the window should do about an event from the pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// The screen changed: repaint.
    Repaint,
    /// Something outside the grid changed (a block, the working directory, progress, the
    /// title): redraw the window's chrome.
    Chrome,
    /// The program set the title.
    Title(String),
    /// The bell rang.
    Bell,
    /// The program ended; the pane is finished.
    Ended(Option<ChildExit>),
    /// The program is holding (or released) the screen.
    Hold(crate::pane::Hold),
    /// Nothing for the window.
    Quiet,
}

/// The drag in progress: how the selection grows from the press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Drag {
    pub granularity: temor_pane::text::Granularity,
    pub shape: temor_pane::text::Shape,
}

/// One pane's state as the window needs it.
/// Logical padding around the grid (PLAN §5: panes snap to whole cells, leftovers are padding).
pub const PADDING_LOGICAL: f32 = 8.0;

pub struct Session {
    pub(super) pane: PaneHandle,
    pub(super) painter: Painter,
    pub keymap: Keymap,
    pub(super) clipboard: Box<dyn Clipboard>,
    pub(super) opener: Box<dyn Opener>,
    pub(super) fit: Option<GridFit>,
    pub(super) scale: Scale,
    pub(super) pixels: PixelSize,
    pub(super) selection: Option<Selection>,
    pub(super) drag: Option<Drag>,
    pub(super) click: Option<ClickState>,
    pub(super) finder: Option<Finder>,
    pub(super) pending: Option<PendingPaste>,
    pub(super) title: String,
    pub(super) lifetime: Lifetime,
    pub(super) notice: Option<String>,
    pub(super) hold: Hold,
    pub(super) hold_generation: u64,
    pub(super) find_text: String,
    pub(super) pending_query: Option<PendingQuery>,
    pub(super) blocks: BlockLog,
    pub(super) selected_block: Option<BlockId>,
    pub(super) cwd: Option<String>,
    pub(super) progress: Option<Progress>,
    pub(super) focus: Focus,
    pub(super) notify_after: NotifyAfter,
    pub(super) limiter: Limiter,
    pub(super) notifier: Box<dyn Notifier>,
    pub(super) clock: Box<dyn Clock>,
    /// Until when the bell's visual mark stays up.
    pub(super) bell_until: Option<Stamp>,
    pub(super) shelf_dir: PathBuf,
}

/// A find query typed but not yet run, and when it was last typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PendingQuery {
    pub(super) text: String,
    pub(super) at: std::time::Duration,
}

/// Whether the program is still running, and how it ended if not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Lifetime {
    Running,
    Ended(Option<ChildExit>),
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("fit", &self.fit)
            .field("scale", &self.scale)
            .field("lifetime", &self.lifetime)
            .finish_non_exhaustive()
    }
}

/// What a session is made from.
pub struct SessionParts {
    pub spec: PaneSpec,
    pub fonts: FontSet,
    pub keymap: Keymap,
    pub clipboard: Box<dyn Clipboard>,
    pub opener: Box<dyn Opener>,
    /// When a finished command is worth a notice.
    pub notify_after: NotifyAfter,
    pub notifier: Box<dyn Notifier>,
    pub clock: Box<dyn Clock>,
    /// Where "Save output" places files (the downloads shelf).
    pub shelf_dir: PathBuf,
}

impl std::fmt::Debug for SessionParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionParts")
            .field("spec", &self.spec)
            .finish_non_exhaustive()
    }
}
