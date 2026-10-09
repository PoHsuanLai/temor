//! The session's lifecycle: opening the program, fitting and painting, and taking the events
//! the program sends.

use temor_core::notify::{Focus, Limiter};
use temor_core::terminal::cwd::cwd_from_url;
use temor_render::{PaneFocus, PixelSize, Scale};
use tokio::sync::mpsc::UnboundedSender;

use super::Session;
use super::blocks::BlockLog;
use super::model::{Lifetime, Notice, PADDING_LOGICAL, SessionParts};
use crate::error::UiError;
use crate::paint::{PaintView, Painter};
use crate::pane::{Hold, PaneEvent, start};

/// How long the bell's visual mark stays up.
pub const BELL_MARK: std::time::Duration = std::time::Duration::from_millis(400);

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
            hold_generation: 0,
            find_text: String::new(),
            pending_query: None,
            blocks: BlockLog::default(),
            selected_block: None,
            cwd: None,
            progress: None,
            focus: Focus::Focused,
            notify_after: parts.notify_after,
            limiter: Limiter::default(),
            notifier: parts.notifier,
            clock: parts.clock,
            bell_until: None,
            shelf_dir: parts.shelf_dir,
        })
    }

    /// The program started holding the screen. Returns the hold's generation, which its timer
    /// passes to [`Session::end_hold`]; a hold never lasts past that timer (PLAN §4b).
    pub fn start_hold(&mut self) -> u64 {
        self.hold_generation += 1;
        self.hold = Hold::Held;
        self.hold_generation
    }

    /// The program released the screen.
    pub fn release_hold(&mut self) {
        self.hold = Hold::Released;
    }

    /// The timer of hold `generation` ran out: releases the hold if it is still the current one.
    /// Returns whether it released anything.
    pub fn end_hold(&mut self, generation: u64) -> bool {
        if self.hold == Hold::Held && self.hold_generation == generation {
            self.hold = Hold::Released;
            return true;
        }
        false
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
            PaneEvent::Bell => {
                let now = self.clock.now();
                self.bell_until = Some(now.after_span(BELL_MARK));
                if let Some(note) = temor_core::notify::bell_notice(self.focus) {
                    self.announce(note);
                }
                Notice::Bell
            }
            PaneEvent::Exited(exit) => {
                self.shell_gone();
                self.lifetime = Lifetime::Ended(exit);
                Notice::Ended(exit)
            }
            PaneEvent::Cwd(url) => {
                self.cwd = cwd_from_url(&url);
                Notice::Chrome
            }
            PaneEvent::Progress(progress) => {
                self.progress = match progress.state {
                    temor_vt::ProgressState::Remove => None,
                    _ => Some(progress),
                };
                Notice::Chrome
            }
            PaneEvent::Prompt(mark) => {
                self.mark(mark);
                Notice::Quiet
            }
            PaneEvent::Anchor(id) => self.anchored(id),
            PaneEvent::Reset => {
                self.terminal_reset();
                Notice::Repaint
            }
            PaneEvent::Notification(note) => {
                let title = sanitise_title(&note.title);
                let body = sanitise_title(&note.body);
                if let Some(desk) = temor_core::notify::program_notice(&title, &body, self.focus) {
                    self.announce(desk);
                }
                Notice::Quiet
            }
            PaneEvent::Clipboard(_) => {
                // TODO(P9): OSC 52 writes need the clipboard's permission, which the pane asks for.
                Notice::Quiet
            }
        }
    }

    /// The bell's visual mark, while it is up.
    pub fn bell_showing(&self) -> bool {
        self.bell_until
            .is_some_and(|until| self.clock.now() < until)
    }

    /// The working directory the shell reported, if any.
    pub fn cwd(&self) -> Option<&str> {
        self.cwd.as_deref()
    }

    /// The progress the program reported (OSC 9;4), while it is shown.
    pub fn progress(&self) -> Option<temor_vt::Progress> {
        self.progress
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
