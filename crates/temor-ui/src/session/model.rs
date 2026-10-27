//! The session's state and the types it reports: the struct, its parts, the notices it
//! gives the window, and the drag and query records. Fields are visible to the session's
//! children only.

use std::path::PathBuf;

use chordkit::Keymap;
use ds_core::time::stamp::Stamp;
use temor_core::notify::Focus;
use temor_pane::settings::NotifyAfter;
use temor_pane::text::Finder;
use temor_pane::text::Selection;
use temor_pty::ChildExit;
use temor_render::{FontSet, GridFit, PixelSize, Scale};

use super::clip::PendingPaste;
use super::clipboard::Clipboard;
use crate::edge::{Clock, Notifier};
use crate::input::ClickState;
use crate::paint::{Inset, Painter};
use crate::platform::Opener;
pub use temor_client::pane::{Lifetime, PaneCore};
use temor_pane::PaneSpec;

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
    /// The shell reported a new working directory.
    Cwd(String),
    /// A command finished (a block ended).
    CommandDone,
    /// The program ended; the pane is finished.
    Ended(Option<ChildExit>),
    /// The program is holding (or released) the screen.
    Hold(temor_pane::Hold),
    /// A program offered, sent or asked for a file (PLAN §9).
    Transfer(temor_pane::TransferEvent),
    /// Nothing for the window.
    Quiet,
}

/// The drag in progress: how the selection grows from the press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Drag {
    pub granularity: temor_pane::text::Granularity,
    pub shape: temor_pane::text::Shape,
}

/// Logical padding around the grid (PLAN §5: panes snap to whole cells, leftovers are padding):
/// the text starts 16 px from the pane's left and right edges and 8 px from its top and bottom.
pub const INSET: Inset = Inset {
    horizontal: PADDING_LOGICAL,
    vertical: 8.0,
};

/// The left (and right) inset alone, for placing things beside the grid's rows.
pub const PADDING_LOGICAL: f32 = 16.0;

/// One pane's state as the window needs it: what the pane is ([`PaneCore`]) and how the window
/// shows it and takes the person's input.
pub struct Session {
    /// The pane itself, apart from how it is shown.
    pub(super) core: PaneCore,
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
    pub(super) notice: Option<String>,
    pub(super) find_text: String,
    pub(super) pending_query: Option<PendingQuery>,
    /// What the person did to the blocks: picked with a click or the arrows, folded away,
    /// bookmarked for the bookmark keys (while the pane lives).
    pub(super) ann: temor_core::session::Annotations,
    /// Where the pointer last was over the grid, for the block it hovers. `None` once it left.
    pub(super) pointer_over: Option<temor_vt::SurfacePoint>,
    /// The link under the pointer as last drawn in its tooltip.
    pub(super) link_hover: Option<String>,
    /// The block list as its view last laid it out, while the pane is drawn as one: where a
    /// point over it lands. `None` while the pane is one grid.
    pub(super) list_map: Option<super::list_hit::ListMap>,
    /// The ground the live screen is drawn on while the pane is a list (the running card's).
    pub(super) live_ground: Option<temor_vt::Rgb>,
    /// Where the next command is typed while the list shows it.
    pub(super) composer: super::composer::Composer,
    pub(super) focus: Focus,
    pub(super) blink: super::blink::Blink,
    /// Until when the bell's visual mark stays up.
    pub(super) bell_until: Option<Stamp>,
    /// A tab or split request a key made, for the window to carry out (see `Session::take_asked`).
    pub(super) asked: Option<temor_pane::keys::Action>,
    /// A file the person asked to see (⌘-click on a path, the View button), for the window to take.
    pub(super) view_ask: Option<super::ViewAsk>,
    /// Where the rows were shown at the last paint (see `room`).
    pub(super) layout: Option<temor_render::RowLayout>,
}

/// A find query typed but not yet run, and when it was last typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PendingQuery {
    pub(super) text: String,
    pub(super) at: std::time::Duration,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("fit", &self.fit)
            .field("scale", &self.scale)
            .field("lifetime", &self.core.lifetime)
            .finish_non_exhaustive()
    }
}

/// What a session is made from.
pub struct SessionParts {
    pub spec: PaneSpec,
    pub fonts: FontSet,
    pub kit: SessionKit,
}

/// What a session is made from besides its pane: the fonts, the keys, the surroundings.
pub struct SessionKit {
    pub keymap: Keymap,
    pub clipboard: Box<dyn Clipboard>,
    pub opener: Box<dyn Opener>,
    /// When a finished command is worth a notice.
    pub notify_after: NotifyAfter,
    /// Whether the cursor blinks.
    pub cursor_blink: temor_pane::settings::CursorBlinking,
    pub notifier: Box<dyn Notifier>,
    pub clock: Box<dyn Clock>,
    /// Where "Save output" places files (the downloads shelf).
    pub shelf_dir: PathBuf,
}

impl std::fmt::Debug for SessionKit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionKit")
            .field("shelf_dir", &self.shelf_dir)
            .finish_non_exhaustive()
    }
}

/// What a session on a program that runs elsewhere is made from (see `temor_pane::start_fed`).
pub struct FedParts {
    pub spec: temor_pane::FedSpec,
    pub fonts: FontSet,
    pub kit: SessionKit,
}

impl std::fmt::Debug for FedParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FedParts")
            .field("spec", &self.spec)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for SessionParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionParts")
            .field("spec", &self.spec)
            .finish_non_exhaustive()
    }
}
