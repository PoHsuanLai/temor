//! The session's lifecycle: opening the program, fitting and painting, and taking the events
//! the program sends.

use temor_core::notify::Focus;
use temor_core::session::{ShellIn, ShellState, shell_step};
use temor_core::vocab::text::display_text;
use temor_pane::text::cwd_from_url;
use temor_render::{Palette, PaneFocus, PixelSize, Scale};

use super::Session;
use super::model::{INSET, Lifetime, Notice};
use super::room::Relaid;
use crate::error::UiError;
use crate::paint::PaintView;
use temor_pane::{Hold, PaneEvent};
use temor_vt::Snapshot;

/// How long the bell's visual mark stays up.
pub const BELL_MARK: std::time::Duration = std::time::Duration::from_millis(400);

impl Session {
    /// The program started holding the screen. Returns the hold's generation, which its timer
    /// passes to [`Session::end_hold`]; a hold never lasts past that timer (PLAN §4b). The frame
    /// shown meanwhile was captured by the pane where the hold began.
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
            // The program is told nothing; its next output shows as it arrives. A terminal that
            // refuses is not the pane's failure: the next release from the program ends it.
            let _ = self.pane.with_term(|term| term.end_hold());
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
            focus: match self.focus {
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
                // The path is shown, so it is cleaned and bounded first (it came from the shell).
                self.cwd = cwd_from_url(&url).map(|path| display_text(&path, CWD_MAX_CHARS));
                self.folder = cwd_from_url(&url).map(std::path::PathBuf::from);
                if let Some(path) = cwd_from_url(&url) {
                    self.shell_step(ShellIn::Cwd(path));
                }
                match &self.cwd {
                    Some(path) => Notice::Cwd(path.clone()),
                    None => Notice::Chrome,
                }
            }
            PaneEvent::ShellReport(text) => {
                // Anything printed in the pane can look like a report; only the shell has the key.
                if let Some(key) = &self.report_key
                    && let Ok(report) = temor_shell::report::read(&text, key)
                {
                    if let Some((_, path)) = report.env.iter().find(|(name, _)| name == "PATH") {
                        self.shell_path = Some(path.clone());
                    }
                    self.shell_step(ShellIn::Reported(report));
                }
                Notice::Quiet
            }
            PaneEvent::Progress(progress) => {
                self.progress = match progress.state {
                    temor_vt::ProgressState::Remove => None,
                    _ => Some(progress),
                };
                Notice::Chrome
            }
            PaneEvent::MarkBegin { seq } => {
                self.mark_begin(seq);
                Notice::Quiet
            }
            PaneEvent::Prompt(mark) => {
                if matches!(mark, temor_vt::PromptEvent::OutputStart { .. }) {
                    self.completion_ended();
                }
                self.mark_event(mark);
                Notice::Quiet
            }
            PaneEvent::Completions(answer) => {
                self.completion_answer(&answer);
                Notice::Chrome
            }
            PaneEvent::Suggested(answered) => {
                self.completion_answered(answered);
                Notice::Chrome
            }
            PaneEvent::Anchor { seq, anchor, at } => self.anchored(seq, anchor, at),
            PaneEvent::Reset => {
                self.terminal_reset();
                // A pane fed from elsewhere is reset when its screen is restored.
                self.find_restored_blocks();
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
            PaneEvent::Transfer(event) => Notice::Transfer(event),
            PaneEvent::Clipboard(write) => self.osc52_write(write),
            PaneEvent::ClipboardRead(read) => self.osc52_read_asked(read),
            // A variant this window does not draw yet: nothing to show.
            _ => Notice::Quiet,
        }
    }

    /// Steps the shell's state on one input.
    fn shell_step(&mut self, input: ShellIn) {
        let state = std::mem::take(&mut self.shell_state);
        self.shell_state = shell_step(state, input).0;
    }

    /// What is kept of this pane when its window goes away: the shell's state and the commands it
    /// ran.
    pub fn checkpoint(&self) -> temor_core::session::Checkpoint {
        temor_core::session::Checkpoint::of(self.shell_state.replay(), self.blocks.store().iter())
            .after(&self.inherited)
    }

    /// Starts from what was kept of the shell this session continues: its state until the shell
    /// reports, and the commands it ran before this window.
    pub fn continue_from(
        &mut self,
        state: temor_core::session::ShellState,
        kept: temor_core::session::Checkpoint,
    ) {
        self.shell_state = state;
        self.inherited = kept.history;
    }

    /// The key the shell's state reports carry, for a shell started elsewhere (the local daemon)
    /// or one that goes on in another pane.
    pub fn set_report_key(&mut self, key: Option<temor_shell::report::ReportKey>) {
        self.report_key = key;
    }

    /// The key of the shell this session shows, if it reports.
    pub fn report_key(&self) -> Option<&temor_shell::report::ReportKey> {
        self.report_key.as_ref()
    }

    /// The commands run before this pane's screen was handed to it, for the blocks found on it.
    pub fn inherit(&mut self, history: Vec<temor_core::session::CommandRecord>) {
        self.inherited = history;
    }

    /// What the shell holds that a new shell would need to look the same.
    pub fn shell_state(&self) -> &ShellState {
        &self.shell_state
    }

    /// The `PATH` the shell last reported, if it has.
    pub fn shell_path(&self) -> Option<&str> {
        self.shell_path.as_deref()
    }

    /// The bell's visual mark, while it is up.
    pub fn bell_showing(&self) -> bool {
        self.bell_until
            .is_some_and(|until| self.clock.now() < until)
    }

    /// The pane's screen and scrollback encoded, to start a pane from later. `None` for a pane on
    /// the alternate screen (a full-screen program's screen is not worth keeping), and when the
    /// encoding is refused or is larger than [`SNAPSHOT_LIMIT`] bytes (the encoding is one block
    /// that cannot be cut to its newest part).
    pub fn snapshot(&self) -> Option<Snapshot> {
        let snapshot = self
            .pane
            .with_term(|term| match term.on_alternate_screen() {
                true => None,
                false => term.snapshot().ok(),
            })?;
        (snapshot.as_bytes().len() <= SNAPSHOT_LIMIT).then_some(snapshot)
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

/// The most bytes a stored snapshot may have (4 MiB).
pub const SNAPSHOT_LIMIT: usize = 4 * 1024 * 1024;

/// Longest title kept; a program's title is shown, so it is bounded.
const TITLE_MAX_CHARS: usize = 200;
/// Longest working directory shown.
const CWD_MAX_CHARS: usize = 512;

/// A program's title with control characters removed and its length bounded.
pub fn sanitise_title(raw: &str) -> String {
    display_text(raw, TITLE_MAX_CHARS)
}
