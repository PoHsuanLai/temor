//! Hearing the pane: each event from its terminal or its program is taken by the core first, and
//! what is left for the window is handed on ([`Heard`]). The desktop's notices are sent from here
//! (PLAN §8.1): a long command finishing, a program's notification, the bell, each through the
//! same rate limit.

use temor_core::block::{Block, format_elapsed};
use temor_core::notify::{Notice as Desk, bell_notice, finished_notice, program_notice};
use temor_pane::PaneEvent;
use temor_pty::ChildExit;

use super::notify::DesktopNote;
use super::{Lifetime, PaneCore, sanitise_title};

/// What an event leaves for the window once the core has taken it.
#[derive(Debug)]
pub enum Heard {
    /// The screen changed: repaint.
    Repaint,
    /// Something outside the grid changed (a block, the folder, progress): redraw the chrome.
    Chrome,
    /// The program set the title.
    Title(String),
    /// The bell rang.
    Bell,
    /// The shell said a new folder.
    Cwd(String),
    /// A command started: the shell running it may be another, which answers no Tab until its
    /// prompt says so.
    CommandStarted,
    /// A command finished.
    CommandDone,
    /// The program ended.
    Ended(Option<ChildExit>),
    /// An event only the window acts on (the composer's completions, the clipboard, a hold on
    /// the screen, a transfer, an agent's report), handed on as it came.
    Window(PaneEvent),
    /// Nothing for the window.
    Quiet,
}

impl PaneCore {
    /// Takes one event from the pane. Output is never blocked on it.
    pub fn hear(&mut self, event: PaneEvent) -> Heard {
        match event {
            PaneEvent::Dirty => {
                // Cleared first: output that arrives while the window paints signals again.
                self.pane.clear_dirty();
                Heard::Repaint
            }
            PaneEvent::Title(title) => Heard::Title(self.retitle(&title)),
            PaneEvent::Bell => {
                if let Some(note) = bell_notice(self.focus) {
                    self.announce(note);
                }
                Heard::Bell
            }
            PaneEvent::Exited(exit) => {
                self.shell_gone();
                self.lifetime = Lifetime::Ended(exit);
                Heard::Ended(exit)
            }
            PaneEvent::Cwd(url) => match self.moved_to(&url) {
                Some(path) => Heard::Cwd(path),
                None => Heard::Chrome,
            },
            PaneEvent::ShellReport(text) => {
                self.reported(&text);
                Heard::Quiet
            }
            PaneEvent::Progress(progress) => {
                self.set_progress(progress);
                Heard::Chrome
            }
            PaneEvent::MarkBegin { seq } => {
                self.mark_begin(seq);
                Heard::Quiet
            }
            PaneEvent::Prompt(mark) => {
                let starts = matches!(mark, temor_vt::PromptEvent::OutputStart { .. });
                self.mark_event(mark);
                match starts {
                    true => Heard::CommandStarted,
                    false => Heard::Quiet,
                }
            }
            PaneEvent::Anchor { seq, anchor, at } => match self.anchored(seq, anchor, at) {
                true => Heard::CommandDone,
                false => Heard::Chrome,
            },
            PaneEvent::Reset => {
                self.terminal_reset();
                // A pane fed from elsewhere is reset when its screen is restored.
                self.find_restored_blocks();
                Heard::Repaint
            }
            PaneEvent::Notification(note) => {
                let title = sanitise_title(&note.title);
                let body = sanitise_title(&note.body);
                if let Some(desk) = program_notice(&title, &body, self.focus) {
                    self.announce(desk);
                }
                Heard::Quiet
            }
            other => Heard::Window(other),
        }
    }

    /// A block finished: notifies when it ran long enough while the pane was unfocused.
    pub(super) fn notify_finished(&mut self, block: &Block) {
        if let Some(note) = finished_notice(block, self.notify_after, self.focus) {
            self.announce(note);
        }
    }

    /// Makes a desktop notice of a core notice, when the limiter lets one through. A notice the
    /// desktop refused is left for the person in [`PaneCore::said`].
    fn announce(&mut self, notice: Desk) {
        let now = self.clock.now();
        if !self.limiter.allow(now) {
            return;
        }
        let note = desktop_note(&notice);
        if let Err(err) = self.notifier.notify(&note) {
            self.said = Some(err.to_string());
        }
    }
}

/// The desktop's words for a core notice (plain text, no developer terms).
pub fn desktop_note(notice: &Desk) -> DesktopNote {
    match notice {
        Desk::CommandFinished {
            outcome,
            took_ms,
            command,
        } => {
            let what = match command.is_empty() {
                true => "The command".to_owned(),
                false => command.clone(),
            };
            let took = format_elapsed(*took_ms);
            match outcome {
                temor_core::block::Outcome::Failed(code) => DesktopNote {
                    title: "Command failed".to_owned(),
                    body: format!("{what} failed with status {} after {took}", code.0),
                },
                _ => DesktopNote {
                    title: "Command finished".to_owned(),
                    body: format!("{what} finished after {took}"),
                },
            }
        }
        Desk::Program { title, body } => DesktopNote {
            title: match title.is_empty() {
                true => "Terminal".to_owned(),
                false => title.clone(),
            },
            body: body.clone(),
        },
        Desk::Bell => DesktopNote {
            title: "Terminal".to_owned(),
            body: "A bell rang in a terminal pane".to_owned(),
        },
    }
}
