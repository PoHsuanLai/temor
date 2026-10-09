//! The session's lifecycle: opening the program, fitting and painting, and taking the events
//! the program sends.

use temor_render::{PaneFocus, PixelSize, Scale};
use tokio::sync::mpsc::UnboundedSender;

use super::Session;
use super::model::{Lifetime, Notice, PADDING_LOGICAL, SessionParts};
use crate::error::UiError;
use crate::paint::{PaintView, Painter};
use crate::pane::{Hold, PaneEvent, start};

impl Session {
    /// Starts the program and its session. Events for the window go to `events`.
    pub fn open(parts: SessionParts, events: UnboundedSender<PaneEvent>) -> Result<Self, UiError> {
        let painter = Painter::new(parts.fonts, parts.spec.palette)?;
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
            notice: None,
            hold: Hold::Released,
            find_text: String::new(),
            pending_query: None,
        })
    }

    /// Whether the program holds the screen. The window sets this from the program's hold events
    /// and its own timeout (PLAN §4b): a hold never lasts past the window's timer.
    pub fn set_hold(&mut self, hold: Hold) {
        self.hold = hold;
    }

    /// A message for the person from the last action, if one is waiting; taking it clears it.
    pub fn take_notice(&mut self) -> Option<String> {
        self.notice.take()
    }

    /// The window's title as the program last set it.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Whether the program has ended.
    pub fn ended(&self) -> bool {
        matches!(self.lifetime, Lifetime::Ended(_))
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
        if changed {
            // Recorded only once the terminal and the program have the new size.
            self.pane.resize(fit.grid, fit.cell)?;
            self.fit = Some(fit);
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
        if self.hold == Hold::Held {
            // The program is holding the screen: keep showing the last frame.
            return Ok(());
        }
        self.painter.paint(&self.pane, view, show)
    }

    /// What one event from the pane means for the window. Output is never blocked on it.
    pub fn absorb(&mut self, event: PaneEvent) -> Notice {
        match event {
            PaneEvent::Dirty => {
                // Cleared first: output that arrives while the window paints signals again.
                self.pane.clear_dirty();
                Notice::Repaint
            }
            PaneEvent::RenderHold(hold) => Notice::Hold(hold),
            PaneEvent::Title(title) => {
                let title = sanitise_title(&title);
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
}

/// Longest title kept; a program's title is shown, so it is bounded.
const TITLE_MAX_CHARS: usize = 200;

/// A program's title with control characters removed and its length bounded.
pub fn sanitise_title(raw: &str) -> String {
    raw.chars()
        .filter(|c| !c.is_control())
        .take(TITLE_MAX_CHARS)
        .collect()
}
