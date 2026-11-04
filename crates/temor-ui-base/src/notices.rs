//! What the window tells the person: each notice is shown until it is closed, then the next.

use std::collections::VecDeque;

use dioxus::prelude::*;

/// The notices waiting to be read, oldest first. A notice already waiting is not queued again,
/// so a failure that repeats (one per pointer move) is closed once.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NoticeQueue(VecDeque<String>);

impl NoticeQueue {
    /// The queue with `text` at its end, unless it is already waiting.
    pub fn with(mut self, text: String) -> Self {
        if !self.0.contains(&text) {
            self.0.push_back(text);
        }
        self
    }

    /// The notice shown now: the oldest.
    pub fn front(&self) -> Option<&str> {
        self.0.front().map(String::as_str)
    }

    /// The queue once the notice shown is closed.
    pub fn closed(mut self) -> Self {
        self.0.pop_front();
        self
    }
}

/// The window's notices, shared by every view that tells the person something.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Notices(Signal<NoticeQueue>);

/// The window's notices, starting with `first` (what the window was started with).
pub fn use_notices(first: impl FnOnce() -> Vec<String>) -> Notices {
    Notices(use_signal(|| {
        first()
            .into_iter()
            .fold(NoticeQueue::default(), NoticeQueue::with)
    }))
}

impl Notices {
    /// Tells the person `text`, after what they have not closed yet.
    pub fn say(self, text: String) {
        self.change(|q| q.with(text));
    }

    /// The notice to show now.
    pub fn shown(self) -> Option<String> {
        self.0.read().front().map(str::to_owned)
    }

    /// Closes the notice shown; the next one shows.
    pub fn close(self) {
        self.change(NoticeQueue::closed);
    }

    fn change(self, step: impl FnOnce(NoticeQueue) -> NoticeQueue) {
        let mut signal = self.0;
        signal.with_mut(|q| *q = step(std::mem::take(q)));
    }
}

#[cfg(test)]
mod tests {
    use super::NoticeQueue;

    fn queue(texts: &[&str]) -> NoticeQueue {
        texts
            .iter()
            .fold(NoticeQueue::default(), |q, t| q.with((*t).to_owned()))
    }

    #[test]
    fn several_notices_are_each_shown_in_the_order_they_were_said() {
        let q = queue(&["first", "second"]);
        assert_eq!(q.front(), Some("first"));
        let q = q.closed();
        assert_eq!(q.front(), Some("second"));
        assert_eq!(q.closed().front(), None);
    }

    #[test]
    fn a_notice_already_waiting_is_not_queued_again() {
        let q = queue(&["failed", "failed", "other", "failed"]);
        assert_eq!(q.closed().closed().front(), None);
    }
}
