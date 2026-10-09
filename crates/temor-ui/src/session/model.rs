//! The session's state and the types it reports: the struct, its parts, the notices it
//! gives the window, and the drag and query records. Fields are visible to the session's
//! children only.

use temor_core::terminal::keymap::Keymap;
use temor_core::terminal::search::Finder;
use temor_core::terminal::selection::Selection;
use temor_pty::ChildExit;
use temor_render::{FontSet, GridFit, PixelSize, Scale};

use super::clip::PendingPaste;
use super::clipboard::Clipboard;
use crate::input::ClickState;
use crate::paint::Painter;
use crate::pane::{Hold, PaneHandle, PaneSpec};
use crate::platform::Opener;

/// What the window should do about an event from the pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// The screen changed: repaint.
    Repaint,
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
    pub granularity: temor_core::terminal::selection::Granularity,
    pub shape: temor_core::terminal::selection::Shape,
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
}

impl std::fmt::Debug for SessionParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionParts")
            .field("spec", &self.spec)
            .finish_non_exhaustive()
    }
}
