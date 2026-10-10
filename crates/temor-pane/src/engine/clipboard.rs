//! The pane's copy of the clipboard for a program that reads it (OSC 52 with `?`, PLAN §6).
//!
//! libghostty wants the answer before the read returns, on the I/O thread, so the window keeps
//! this copy up to date (when the pane gains focus, and when the person allows reads) and the
//! terminal answers from it. A program that reads while the person has not been asked gets
//! nothing; the window is told to ask, and the answer is remembered for the pane's life.

use std::sync::{Arc, Mutex, MutexGuard};

use temor_vt::{ClipboardAnswer, ClipboardLocation, ClipboardRead, ClipboardSource};

/// The longest clipboard text kept for reads (1 MiB). Longer text is not served: a program is
/// told the clipboard is empty rather than given part of it.
pub const MIRROR_LIMIT: usize = 1024 * 1024;

/// Whether the person has let the program read the clipboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ReadPolicy {
    /// Not asked yet: the first read asks.
    #[default]
    Ask,
    /// Allowed: reads get the copy.
    Allowed,
    /// Refused: reads get nothing.
    Refused,
}

#[derive(Debug, Default)]
struct Mirror {
    policy: ReadPolicy,
    text: Option<String>,
}

/// The clipboard copy and the read policy, shared between the window and the terminal.
/// `Send` and `Sync`; clones share the same state.
#[derive(Debug, Clone, Default)]
pub struct ClipboardMirror(Arc<Mutex<Mirror>>);

impl ClipboardMirror {
    fn lock(&self) -> MutexGuard<'_, Mirror> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The person's answer so far.
    pub fn policy(&self) -> ReadPolicy {
        self.lock().policy
    }

    /// Records the person's answer.
    pub fn set_policy(&self, policy: ReadPolicy) {
        self.lock().policy = policy;
    }

    /// Replaces the copy; `None` when the clipboard has no text or could not be read.
    pub fn set_text(&self, text: Option<String>) {
        self.lock().text = text.filter(|t| t.len() <= MIRROR_LIMIT);
    }
}

impl ClipboardSource for ClipboardMirror {
    fn read(&self, read: &ClipboardRead) -> ClipboardAnswer {
        let mirror = self.lock();
        match (mirror.policy, read.location) {
            (ReadPolicy::Ask, _) => ClipboardAnswer::Ask,
            (ReadPolicy::Refused, _) => ClipboardAnswer::Denied,
            (ReadPolicy::Allowed, ClipboardLocation::Standard) => mirror
                .text
                .clone()
                .map_or(ClipboardAnswer::Empty, ClipboardAnswer::Text),
            // Only the standard clipboard is mirrored.
            (ReadPolicy::Allowed, _) => ClipboardAnswer::Empty,
        }
    }
}
