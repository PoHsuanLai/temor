//! What the pane's own events change of it: its title, its folder, its shell's state, its
//! progress, a hold on its screen, and what is kept of it when the window goes.

use temor_blocks::session::{Checkpoint, SNAPSHOT_LIMIT, ShellIn, ShellState};
use temor_core::vocab::text::display_text;
use temor_pane::RenderHold;
use temor_shell::cwd_from_url;
use temor_vt::{Progress, ProgressState, Snapshot};

use super::{Lifetime, PaneCore};

/// Longest title kept; a program's title is shown, so it is bounded.
const TITLE_MAX_CHARS: usize = 200;
/// Longest working directory shown.
const CWD_MAX_CHARS: usize = 512;

/// A program's title with control characters removed and its length bounded.
pub fn sanitise_title(raw: &str) -> String {
    display_text(raw, TITLE_MAX_CHARS)
}

impl PaneCore {
    /// The program started holding the screen. Returns the hold's generation, which its timer
    /// passes to [`PaneCore::end_hold`]; a hold never lasts past that timer (PLAN §4b).
    pub fn start_hold(&mut self) -> u64 {
        self.hold_generation += 1;
        self.hold = RenderHold::Held;
        self.hold_generation
    }

    /// The program released the screen.
    pub fn release_hold(&mut self) {
        self.hold = RenderHold::Released;
    }

    /// The timer of hold `generation` ran out: releases the hold if it is still the current one.
    /// Returns whether it released anything.
    pub fn end_hold(&mut self, generation: u64) -> bool {
        if self.hold == RenderHold::Held && self.hold_generation == generation {
            self.hold = RenderHold::Released;
            // The program is told nothing; its next output shows as it arrives. A terminal that
            // refuses is not the pane's failure: the next release from the program ends it.
            let _ = self.pane.with_term(|term| term.end_hold());
            return true;
        }
        false
    }

    /// Whether the program has ended.
    pub fn ended(&self) -> bool {
        matches!(self.lifetime, Lifetime::Ended(_))
    }

    /// The program set its title. Returns the title as kept.
    pub fn retitle(&mut self, raw: &str) -> String {
        self.title = sanitise_title(raw);
        self.title.clone()
    }

    /// The shell said its folder (OSC 7). Returns the folder as shown, if the address was one.
    pub fn moved_to(&mut self, url: &str) -> Option<String> {
        let path = cwd_from_url(url);
        // The path is shown, so it is cleaned and bounded first (it came from the shell).
        self.cwd = path
            .as_deref()
            .map(|path| display_text(path, CWD_MAX_CHARS));
        self.folder = path.as_deref().map(std::path::PathBuf::from);
        if let Some(path) = path {
            self.shell_state.apply(ShellIn::Cwd(path));
        }
        self.cwd.clone()
    }

    /// The shell reported its state. Anything printed in the pane can look like a report; only
    /// the shell has the key, so a report without it is dropped.
    pub fn reported(&mut self, text: &str) {
        let Some(key) = &self.report_key else {
            return;
        };
        let Ok(report) = temor_shell::report::read(text, key) else {
            return;
        };
        if let Some((_, path)) = report.env.iter().find(|(name, _)| name == "PATH") {
            self.shell_path = Some(path.clone());
        }
        self.shell_state.apply(ShellIn::Reported(report));
    }

    /// The program reported its progress (OSC 9;4).
    pub fn set_progress(&mut self, progress: Progress) {
        self.progress = match progress.state {
            ProgressState::Remove => None,
            _ => Some(progress),
        };
    }

    /// What is kept of this pane when its window goes away: the shell's state and the commands it
    /// ran.
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint::of(self.shell_state.replay(), self.blocks.store().iter()).after(&self.inherited)
    }

    /// Starts from what was kept of the shell this pane continues: its state until the shell
    /// reports, and the commands it ran before this window.
    pub fn continue_from(&mut self, state: ShellState, kept: Checkpoint) {
        self.shell_state = state;
        self.inherited = kept.history;
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
}
