//! The session's lifecycle: opening the program, fitting and painting, and taking the events
//! the program sends.

use temor_core::notify::Focus;
use temor_core::session::ShellState;
use temor_render::{Palette, PaneFocus, PixelSize, Scale};

use super::Session;
use super::model::{INSET, Notice};
use super::room::Relaid;
use crate::error::UiError;
use crate::paint::PaintView;
use temor_pane::PaneEvent;
use temor_panes::Heard;
use temor_vt::Snapshot;

/// How long the bell's visual mark stays up.
pub const BELL_MARK: std::time::Duration = std::time::Duration::from_millis(400);

impl Session {
    /// The program started holding the screen. Returns the hold's generation, which its timer
    /// passes to [`Session::end_hold`]; a hold never lasts past that timer (PLAN §4b). The frame
    /// shown meanwhile was captured by the pane where the hold began.
    pub fn start_hold(&mut self) -> u64 {
        self.core().start_hold()
    }

    /// The program released the screen.
    pub fn release_hold(&mut self) {
        self.core().release_hold();
    }

    /// The timer of hold `generation` ran out: releases the hold if it is still the current one.
    /// Returns whether it released anything.
    pub fn end_hold(&mut self, generation: u64) -> bool {
        self.core().end_hold(generation)
    }

    /// A message for the person from the last action, if one is waiting; taking it clears it.
    pub fn take_notice(&mut self) -> Option<String> {
        self.notice.take().or_else(|| self.core().said.take())
    }

    /// The window's title as the program last set it.
    pub fn title(&self) -> String {
        self.core().title.clone()
    }

    /// Whether the program has ended.
    pub fn ended(&self) -> bool {
        self.core().ended()
    }

    /// The pane's size and scale changed. The grid is refitted; when it changed, the terminal and
    /// the program are resized too. Returns whether the grid changed.
    pub fn layout(&mut self, pixels: PixelSize, scale: Scale) -> Result<bool, UiError> {
        self.pixels = pixels;
        self.scale = scale;
        if pixels.width.0 == 0 || pixels.height.0 == 0 {
            return Ok(false);
        }
        let fit = self.painter.fit(pixels, scale, INSET);
        let changed = self.fit.map(|f| (f.grid, f.cell)) != Some((fit.grid, fit.cell));
        if changed {
            // Recorded only once the terminal and the program have the new size.
            self.pane.resize(fit.grid, fit.cell)?;
            self.fit = Some(fit);
            self.relayout();
        }
        Ok(changed)
    }

    /// Makes `palette` the grid's colours: the terminal's defaults (so a program that reads them
    /// sees the new ones) and the painter's. The next paint redraws the whole grid.
    pub fn set_palette(&mut self, palette: Palette) -> Result<(), UiError> {
        self.pane.with_term(|term| term.set_palette(&palette))?;
        self.painter.set_palette(palette);
        Ok(())
    }

    /// Draws the pane when it has a size, handing its pixels to `show`, and lays its rows out
    /// around the room under blocks with pictures. Whether the rows moved.
    pub fn paint(&mut self, show: impl FnOnce(&[u8], PixelSize)) -> Result<Relaid, UiError> {
        if self.pixels.width.0 == 0 || self.pixels.height.0 == 0 {
            return Ok(Relaid::Same);
        }
        let blink = self.blink_now();
        self.blink.painted = blink;
        let view = PaintView {
            pixels: self.pixels,
            scale: self.scale,
            inset: INSET,
            focus: match self.core().focus {
                Focus::Focused => PaneFocus::Focused,
                Focus::Unfocused => PaneFocus::Unfocused,
            },
            blink,
            selection: None,
            ground: self.live_ground,
        };
        // While the program holds the screen the painter draws the frame captured where the
        // hold began (see `temor_pane::PaintSource`).
        self.painter.paint(&self.pane, view, show)?;
        Ok(self.relayout())
    }

    /// What one event from the pane means for the window. Output is never blocked on it. The
    /// pane's core takes it first (`PaneCore::hear`); the window does the rest ([`Session::take`]).
    pub fn absorb(&mut self, event: PaneEvent) -> Notice {
        let heard = self.core().hear(event);
        self.take(heard)
    }

    /// The pane's core takes one event; what is left is for [`Session::take`].
    pub fn hear(&mut self, event: PaneEvent) -> Heard {
        self.core().hear(event)
    }

    /// What is left of an event for the window once the pane's core has taken it.
    pub fn take(&mut self, heard: Heard) -> Notice {
        match heard {
            Heard::Repaint | Heard::Reset => Notice::Repaint,
            Heard::Chrome | Heard::Marked => Notice::Chrome,
            Heard::Title(title) => Notice::Title(title),
            Heard::Bell => {
                let now = self.core().clock.now();
                self.bell_until = Some(now.after_span(BELL_MARK));
                Notice::Bell
            }
            Heard::Cwd(path) => Notice::Cwd(path),
            Heard::CommandStarted => {
                self.completion_ended();
                Notice::Quiet
            }
            Heard::CommandDone => Notice::CommandDone,
            Heard::Ended(exit) => Notice::Ended(exit),
            Heard::Quiet | Heard::PromptShown | Heard::Notified { .. } => Notice::Quiet,
            Heard::Window(event) => match event {
                PaneEvent::RenderHold(hold) => Notice::Hold(hold),
                PaneEvent::Completions(answer) => {
                    self.completion_answer(&answer);
                    Notice::Chrome
                }
                PaneEvent::Suggested(answered) => {
                    self.completion_answered(answered);
                    Notice::Chrome
                }
                PaneEvent::Transfer(event) => Notice::Transfer(event),
                PaneEvent::Clipboard(write) => self.osc52_write(write),
                PaneEvent::ClipboardRead(read) => self.osc52_read_asked(read),
                // A variant this window does not draw yet: nothing to show.
                _ => Notice::Quiet,
            },
        }
    }

    /// What is kept of this pane when its window goes away: the shell's state and the commands it
    /// ran.
    pub fn checkpoint(&self) -> temor_core::session::Checkpoint {
        self.core().checkpoint()
    }

    /// Starts from what was kept of the shell this session continues: its state until the shell
    /// reports, and the commands it ran before this window.
    pub fn continue_from(
        &mut self,
        state: temor_core::session::ShellState,
        kept: temor_core::session::Checkpoint,
    ) {
        self.core().continue_from(state, kept);
    }

    /// The key the shell's state reports carry, for a shell started elsewhere (the local daemon)
    /// or one that goes on in another pane.
    pub fn set_report_key(&mut self, key: Option<temor_shell::report::ReportKey>) {
        self.core().report_key = key;
    }

    /// The key of the shell this session shows, if it reports.
    pub fn report_key(&self) -> Option<temor_shell::report::ReportKey> {
        self.core().report_key.clone()
    }

    /// The commands run before this pane's screen was handed to it, for the blocks found on it.
    pub fn inherit(&mut self, history: Vec<temor_core::session::CommandRecord>) {
        self.core().inherit(history);
    }

    /// What the shell holds that a new shell would need to look the same.
    pub fn shell_state(&self) -> ShellState {
        self.core().shell_state.clone()
    }

    /// The `PATH` the shell last reported, if it has.
    pub fn shell_path(&self) -> Option<String> {
        self.core().shell_path.clone()
    }

    /// The bell's visual mark, while it is up.
    pub fn bell_showing(&self) -> bool {
        self.bell_until
            .is_some_and(|until| self.core().clock.now() < until)
    }

    /// The pane's screen and scrollback encoded, to start a pane from later. `None` for a pane on
    /// the alternate screen (a full-screen program's screen is not worth keeping), and when the
    /// encoding is refused or is larger than [`SNAPSHOT_LIMIT`] bytes (the encoding is one block
    /// that cannot be cut to its newest part).
    pub fn snapshot(&self) -> Option<Snapshot> {
        self.core().snapshot()
    }

    /// The working directory the shell reported, if any.
    pub fn cwd(&self) -> Option<String> {
        self.core().cwd.clone()
    }

    /// The progress the program reported (OSC 9;4), while it is shown.
    pub fn progress(&self) -> Option<temor_vt::Progress> {
        self.core().progress
    }
}

pub use temor_panes::{SNAPSHOT_LIMIT, sanitise_title};
