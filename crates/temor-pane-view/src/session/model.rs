//! The session's state and the types it reports: the struct, its parts, the notices it
//! gives the window, and the drag and query records. Fields are visible to the session's
//! children only.

use std::path::PathBuf;

use chordkit::Keymap;
use ds_core::time::stamp::Stamp;
use temor_panes::pane::settings::NotifyAfter;
use temor_panes::pane::text::Finder;
use temor_panes::pane::text::Selection;
use temor_panes::pty::ChildExit;
use temor_panes::render::{FontSet, GridFit, PixelSize, Scale};

use super::clip::PendingPaste;
use super::clipboard::Clipboard;
use crate::input::ClickState;
use crate::paint::{Inset, Painter};
use crate::platform::Opener;
pub use temor_panes::PaneCore;
use temor_panes::clock::Clock;
use temor_panes::notify::Notifier;
use temor_panes::pane::PaneSpec;
use temor_panes::{Locked, Shared};

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
    Hold(temor_panes::pane::RenderHold),
    /// A program offered, sent or asked for a file (PLAN §9).
    Transfer(temor_panes::pane::TransferEvent),
    /// Nothing for the window.
    Quiet,
}

/// The drag in progress: how the selection grows from the press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Drag {
    pub granularity: temor_panes::pane::text::Granularity,
    pub shape: temor_panes::pane::text::Shape,
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
    /// The pane itself, apart from how it is shown, shared with the client's thread that hears
    /// it (see [`Session::core`]).
    pub(super) core: Shared,
    /// The pane's engine, as the core holds it: painting and typing need no lock on the core.
    pub(super) pane: std::sync::Arc<temor_panes::pane::PaneHandle>,
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
    pub(super) ann: temor_client::model::session::Annotations,
    /// Where the pointer last was over the grid, for the block it hovers. `None` once it left.
    pub(super) pointer_over: Option<temor_panes::vt::SurfacePoint>,
    /// The link under the pointer as last drawn in its tooltip.
    pub(super) link_hover: Option<String>,
    /// The block list as its view last laid it out, while the pane is drawn as one: where a
    /// point over it lands. `None` while the pane is one grid.
    pub(super) list_map: Option<super::list_hit::ListMap>,
    /// The ground the live screen is drawn on while the pane is a list (the running card's).
    pub(super) live_ground: Option<temor_panes::vt::Rgb>,
    /// Where the next command is typed while the list shows it.
    pub(super) composer: super::composer::Composer,
    pub(super) blink: super::blink::Blink,
    /// Until when the bell's visual mark stays up.
    pub(super) bell_until: Option<Stamp>,
    /// A tab or split request a key made, for the window to carry out (see `Session::take_asked`).
    pub(super) asked: Option<temor_panes::pane::keys::Action>,
    /// A file the person asked to see (⌘-click on a path, the View button), for the window to take.
    pub(super) view_ask: Option<super::ViewAsk>,
    /// The commands the session ran before it was reopened, shown above the pane's own.
    pub(super) kept: Vec<temor_client::model::session::list::KeptCard>,
    /// Where the rows were shown at the last paint (see `room`).
    pub(super) layout: Option<temor_panes::render::RowLayout>,
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
            .field("lifetime", &self.core().lifetime)
            .finish_non_exhaustive()
    }
}

/// The longest the window waits for a pane's core. The client's thread holds it only while it
/// hears one event, so a wait this long means the core was locked twice on one thread.
const CORE_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

/// `core`, locked; for a statement that also borrows another of the session's fields.
pub(super) fn locked(core: &Shared) -> Locked<'_> {
    core.try_lock_for(CORE_WAIT)
        .expect("a pane's core stayed locked: it was locked twice on one thread")
}

impl Session {
    /// The pane's core, locked. Held for one statement at a time: a core locked again on the
    /// same thread before the first is let go waits for itself.
    pub(super) fn core(&self) -> Locked<'_> {
        locked(&self.core)
    }

    /// The pane's core as the client's thread holds it, to make the pane known to that thread.
    pub fn shared_core(&self) -> &Shared {
        &self.core
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
    pub cursor_blink: bool,
    pub notifier: Box<dyn Notifier>,
    pub clock: Box<dyn Clock>,
    /// Where "Save output" places files (the downloads shelf).
    pub shelf_dir: PathBuf,
}

/// What the window keeps of a [`SessionKit`] once the pane's part is handed to the client.
pub(super) struct WindowKit {
    pub keymap: Keymap,
    pub clipboard: Box<dyn Clipboard>,
    pub opener: Box<dyn Opener>,
    pub cursor_blink: bool,
}

impl SessionKit {
    /// The pane's part and the window's.
    pub(super) fn split(self) -> (temor_panes::PaneKit, WindowKit) {
        let SessionKit {
            keymap,
            clipboard,
            opener,
            notify_after,
            cursor_blink,
            notifier,
            clock,
            shelf_dir,
        } = self;
        (
            temor_panes::PaneKit {
                notify_after,
                notifier,
                clock,
                shelf_dir,
            },
            WindowKit {
                keymap,
                clipboard,
                opener,
                cursor_blink,
            },
        )
    }
}

impl std::fmt::Debug for SessionKit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionKit")
            .field("shelf_dir", &self.shelf_dir)
            .finish_non_exhaustive()
    }
}

/// What a session on a program that runs elsewhere is made from (see `temor_panes::pane::start_fed`).
pub struct FedParts {
    pub spec: temor_panes::pane::FedSpec,
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
