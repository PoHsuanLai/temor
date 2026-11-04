//! Hearing the pane: each event from its terminal or its program is taken by the core first, and
//! what is left for the window is handed on ([`Heard`]). The desktop's notices are sent from here
//! (PLAN §8.1): a long command finishing, a program's notification, the bell, each through the
//! same rate limit.

use temor_blocks::block::{Block, format_elapsed};
use temor_blocks::notify::{Notice as Desk, bell_notice, finished_notice, program_notice};
use temor_pane::PaneEvent;
use temor_pty::ChildExit;

use super::marks::Finished;
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
    /// A program's notification (OSC 9, OSC 777), its words cleaned; the desktop was told.
    Notified { title: String, body: String },
    /// The shell said a new folder.
    Cwd(String),
    /// The shell showed its prompt.
    PromptShown,
    /// A command started: the shell running it may be another, which answers no Tab until its
    /// prompt says so.
    CommandStarted,
    /// A mark was placed (a prompt, a command's start or end), and no command finished.
    Marked,
    /// A command finished.
    CommandDone,
    /// The terminal was reset (or a fed pane's screen restored): repaint.
    Reset,
    /// The program ended.
    Ended(Option<ChildExit>),
    /// An event only the window acts on (the composer's completions, the clipboard, a hold on
    /// the screen, a transfer, an agent's report), handed on as it came.
    Window(PaneEvent),
    /// Commands that finished, as history is told of them; sent before the event's own.
    Finished(Vec<Finished>),
    /// The desktop refused a notice: why, for the person.
    Refused(String),
    /// Nothing for the window.
    Quiet,
}

/// What hearing an event asks of the edges, besides what it leaves for the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneEffect {
    /// Show this notice on the desktop (the rate limit already let it through).
    Notify(DesktopNote),
    /// A command finished.
    Finished(Finished),
}

impl PaneCore {
    /// Takes one event from the pane: what it leaves for the window, and what the edges are to
    /// do (desktop notices, finished commands), in order. Output is never blocked on it.
    pub fn hear(&mut self, event: PaneEvent) -> (Heard, Vec<PaneEffect>) {
        let heard = self.take_event(event);
        (heard, std::mem::take(&mut self.effects))
    }

    fn take_event(&mut self, event: PaneEvent) -> Heard {
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
                let heard = match mark {
                    temor_vt::PromptEvent::OutputStart { .. } => Heard::CommandStarted,
                    temor_vt::PromptEvent::PromptStart(_) => Heard::PromptShown,
                    _ => Heard::Quiet,
                };
                self.mark_event(mark);
                heard
            }
            PaneEvent::Anchor { seq, anchor, at } => match self.anchored(seq, anchor, at) {
                true => Heard::CommandDone,
                false => Heard::Marked,
            },
            PaneEvent::Reset => {
                self.terminal_reset();
                // A pane fed from elsewhere is reset when its screen is restored.
                self.find_restored_blocks();
                Heard::Reset
            }
            PaneEvent::Notification(note) => {
                let title = sanitise_title(&note.title);
                let body = sanitise_title(&note.body);
                if let Some(desk) = program_notice(&title, &body, self.focus) {
                    self.announce(desk);
                }
                Heard::Notified { title, body }
            }
            other => Heard::Window(other),
        }
    }

    /// A block finished: history is told, and the desktop when it ran long enough while the
    /// pane was unfocused.
    pub(super) fn command_finished(&mut self, block: &Block) {
        if let Some(done) = Finished::of(block) {
            self.effects.push(PaneEffect::Finished(done));
        }
        if let Some(note) = finished_notice(block, self.notify_after, self.focus) {
            self.announce(note);
        }
    }

    /// A desktop notice of a core notice, when the limiter lets one through.
    fn announce(&mut self, notice: Desk) {
        let now = self.clock.now();
        if self.limiter.allow(now) {
            self.effects.push(PaneEffect::Notify(desktop_note(&notice)));
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
                temor_blocks::block::Outcome::Failed(code) => DesktopNote {
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
