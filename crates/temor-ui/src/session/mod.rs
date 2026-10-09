//! One pane as the window drives it: the engine's handle, the painter, the selection, the find
//! bar and the paste confirmation. The window sends it events and reads back what to show; it
//! never touches the terminal or the PTY itself. Dioxus is not used here.

mod clip;
mod clipboard;
mod find;
mod keys;
mod pointer;

pub use clip::PendingPaste;
pub use clipboard::{Clipboard, ClipboardFailed};

use temor_core::terminal::keymap::Keymap;
use temor_core::terminal::search::Finder;
use temor_core::terminal::selection::Selection;
use temor_pty::ChildExit;
use temor_render::{FontSet, GridFit, PaneFocus, PixelSize, Scale};
use tokio::sync::mpsc::UnboundedSender;

use crate::error::UiError;
use crate::input::ClickState;
use crate::paint::{PaintView, Painter};
use crate::pane::{PaneEvent, PaneHandle, PaneSpec, start};
use crate::platform::Opener;

/// Logical padding around the grid (PLAN §5: panes snap to whole cells, leftovers are padding).
pub const PADDING_LOGICAL: f32 = 8.0;

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
pub struct Session {
    pane: PaneHandle,
    painter: Painter,
    keymap: Keymap,
    clipboard: Box<dyn Clipboard>,
    opener: Box<dyn Opener>,
    fit: Option<GridFit>,
    scale: Scale,
    pixels: PixelSize,
    selection: Option<Selection>,
    drag: Option<Drag>,
    click: Option<ClickState>,
    finder: Option<Finder>,
    pending: Option<PendingPaste>,
    title: String,
    lifetime: Lifetime,
}

/// Whether the program is still running, and how it ended if not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lifetime {
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

impl Session {
    /// Starts the program and its session. Events for the window go to `events`.
    pub fn open(parts: SessionParts, events: UnboundedSender<PaneEvent>) -> Result<Self, UiError> {
        let painter = Painter::new(parts.fonts, parts.spec.palette);
        let pane = start(parts.spec, events)?;
        Ok(Self {
            pane,
            painter,
            keymap: parts.keymap,
            clipboard: parts.clipboard,
            opener: parts.opener,
            fit: None,
            scale: Scale::new(1.0),
            pixels: PixelSize::new(0, 0),
            selection: None,
            drag: None,
            click: None,
            finder: None,
            pending: None,
            title: String::new(),
            lifetime: Lifetime::Running,
        })
    }

    /// The pane's engine handle, for the window's painting loop and tests.
    pub fn pane(&self) -> &PaneHandle {
        &self.pane
    }

    /// The grid as last fitted to the pane (`None` before the first layout).
    pub fn fit(&self) -> Option<GridFit> {
        self.fit
    }

    /// The screen's text, row by row, as the terminal holds it (for tests and the probe).
    pub fn screen_text(&self) -> String {
        self.pane.with_term(|term| {
            let rows = term.screen_rows().unwrap_or(0);
            (0..rows)
                .map(|row| term.row_text(row).unwrap_or_default())
                .collect::<Vec<_>>()
                .join("\n")
        })
    }

    /// The window's title as the program last set it.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Whether the program has ended.
    pub fn ended(&self) -> bool {
        matches!(self.lifetime, Lifetime::Ended(_))
    }

    /// The find bar's state, if it is open.
    pub fn finder(&self) -> Option<&Finder> {
        self.finder.as_ref()
    }

    /// A paste waiting for the person's confirmation.
    pub fn pending_paste(&self) -> Option<&PendingPaste> {
        self.pending.as_ref()
    }

    /// The keymap the pane routes keys with.
    pub fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    /// The pane's size and scale changed. The grid is refitted; when it changed, the terminal and
    /// the program are resized too. Returns whether the grid changed.
    pub fn layout(&mut self, pixels: PixelSize, scale: Scale) -> Result<bool, UiError> {
        self.pixels = pixels;
        self.scale = scale;
        if pixels.width.0 == 0 || pixels.height.0 == 0 {
            return Ok(false);
        }
        let fit = self.painter.fit(pixels, scale, PADDING_LOGICAL);
        let changed = self.fit.map(|f| (f.grid, f.cell)) != Some((fit.grid, fit.cell));
        self.fit = Some(fit);
        if changed {
            self.pane.resize(fit.grid, fit.cell)?;
        }
        Ok(changed)
    }

    /// Draws the pane when it has a size, handing its pixels to `show`.
    pub fn paint(&mut self, show: impl FnOnce(&[u8], PixelSize)) -> Result<(), UiError> {
        if self.pixels.width.0 == 0 || self.pixels.height.0 == 0 {
            return Ok(());
        }
        let view = PaintView {
            pixels: self.pixels,
            scale: self.scale,
            padding_logical: PADDING_LOGICAL,
            focus: PaneFocus::Focused,
            selection: None,
        };
        let painter = &mut self.painter;
        self.pane.with_term(|term| painter.paint(term, view, show))
    }

    /// What one event from the pane means for the window. Output is never blocked on it.
    pub fn absorb(&mut self, event: PaneEvent) -> Notice {
        match event {
            PaneEvent::Dirty => Notice::Repaint,
            PaneEvent::Title(title) => {
                self.title = title.clone();
                Notice::Title(title)
            }
            PaneEvent::Bell => Notice::Bell,
            PaneEvent::Exited(exit) => {
                self.lifetime = Lifetime::Ended(exit);
                Notice::Ended(exit)
            }
            PaneEvent::Cwd(_)
            | PaneEvent::Clipboard(_)
            | PaneEvent::Notification(_)
            | PaneEvent::Prompt(_) => {
                // TODO(P2/P9): blocks, the working directory, OSC 52 writes and notifications.
                Notice::Quiet
            }
        }
    }

    /// The viewport's first screen row (what the pane shows at the top).
    fn viewport_top(&self) -> Result<u32, UiError> {
        Ok(self.pane.with_term(|term| term.viewport())?.0)
    }
}
