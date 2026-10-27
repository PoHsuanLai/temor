//! Notices from the session (PLAN §8.1): a long command finishing, a program's notification, the
//! bell. Each goes through the same rate limit to the notifier edge.

use temor_core::notify::Focus;
use temor_pane::settings::NotifyAfter;

use super::super::Session;

impl Session {
    /// The focus the window reports (a pane that is not focused notifies).
    pub fn set_focus(&mut self, focus: Focus) {
        self.core().focus = focus;
        self.wake_cursor();
        // Something may have been copied elsewhere meanwhile: a program that reads the
        // clipboard (OSC 52) gets what is there now.
        if focus == Focus::Focused {
            self.refresh_clipboard_copy();
        }
    }

    /// Changes how long a command must run before its finish is announced, from the next one.
    pub fn set_notify_after(&mut self, after: NotifyAfter) {
        self.core().notify_after = after;
    }

    /// The notify setting the session uses.
    pub fn notify_after(&self) -> NotifyAfter {
        self.core().notify_after
    }
}

pub use temor_panes::desktop_note;
