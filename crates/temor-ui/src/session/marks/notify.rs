//! Notices from the session (PLAN §8.1): a long command finishing, a program's notification, the
//! bell. Each goes through the same rate limit to the notifier edge.

use temor_core::block::{Block, format_elapsed};
use temor_core::notify::{Focus, Notice as Desk, finished_notice};
use temor_pane::settings::NotifyAfter;

use super::super::Session;
use crate::edge::DesktopNote;

impl Session {
    /// A block finished: notifies when it ran long enough while the pane was unfocused.
    pub(super) fn notify_finished(&mut self, block: &Block) {
        if let Some(note) = finished_notice(block, self.core.notify_after, self.focus) {
            self.announce(note);
        }
    }

    /// Makes a desktop notice of a core notice, when the limiter lets one through.
    pub(in crate::session) fn announce(&mut self, notice: Desk) {
        let now = self.core.clock.now();
        if !self.core.limiter.allow(now) {
            return;
        }
        let note = desktop_note(&notice);
        if let Err(err) = self.core.notifier.notify(&note) {
            self.notice = Some(err.to_string());
        }
    }

    /// The focus the window reports (a pane that is not focused notifies).
    pub fn set_focus(&mut self, focus: Focus) {
        self.focus = focus;
        self.wake_cursor();
        // Something may have been copied elsewhere meanwhile: a program that reads the
        // clipboard (OSC 52) gets what is there now.
        if focus == Focus::Focused {
            self.refresh_clipboard_copy();
        }
    }

    /// Changes how long a command must run before its finish is announced, from the next one.
    pub fn set_notify_after(&mut self, after: NotifyAfter) {
        self.core.notify_after = after;
    }

    /// The notify setting the session uses.
    pub fn notify_after(&self) -> NotifyAfter {
        self.core.notify_after
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
