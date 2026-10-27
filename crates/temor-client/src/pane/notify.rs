//! The desktop's notifications (PLAN §8.1). The session decides what is worth a notice; this edge
//! shows it. Nothing here runs on the render path.

use std::cell::RefCell;
use std::rc::Rc;

use thiserror::Error;

/// A notice as the desktop shows it: a title and a body, both plain text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopNote {
    pub title: String,
    pub body: String,
}

/// The notice could not be shown.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("the notification could not be shown: {0}")]
pub struct NotifyFailed(pub String);

/// Shows a notice on the desktop.
pub trait Notifier {
    fn notify(&self, note: &DesktopNote) -> Result<(), NotifyFailed>;
}

/// The default until quire's desktop feature provides a notification service (quire v0.2.30 has
/// none: its notifications are in sill's shell kit). Notices are dropped.
// TODO(quire-desktop): the freedesktop notification service through ds-desktop, with the
// notification's activation raising the window (PLAN §8.1). Until then nothing is shown.
#[derive(Debug, Default)]
pub struct NoopNotifier;

impl Notifier for NoopNotifier {
    fn notify(&self, _note: &DesktopNote) -> Result<(), NotifyFailed> {
        Ok(())
    }
}

/// A notifier that keeps what it was given, for tests and for the window's log.
#[derive(Debug, Default, Clone)]
pub struct RecordingNotifier {
    sent: Rc<RefCell<Vec<DesktopNote>>>,
}

impl RecordingNotifier {
    /// The notices sent so far, oldest first.
    pub fn sent(&self) -> Vec<DesktopNote> {
        self.sent.borrow().clone()
    }
}

impl Notifier for RecordingNotifier {
    fn notify(&self, note: &DesktopNote) -> Result<(), NotifyFailed> {
        self.sent.borrow_mut().push(note.clone());
        Ok(())
    }
}
