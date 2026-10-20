//! The clipboard as the pane uses it: copy writes text, paste reads it. The desktop's clipboard
//! is the real one; tests use a memory clipboard.

use thiserror::Error;

/// The clipboard could not be used.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("the clipboard is not available: {0}")]
pub struct ClipboardFailed(pub String);

/// Reads and writes the desktop clipboard's text.
pub trait Clipboard {
    fn write(&self, text: &str) -> Result<(), ClipboardFailed>;
    fn read(&self) -> Result<String, ClipboardFailed>;
    /// Puts `text` on the primary selection (the one a middle click pastes), where the desktop
    /// has one. A clipboard without one refuses.
    fn write_primary(&self, text: &str) -> Result<(), ClipboardFailed> {
        let _ = text;
        Err(ClipboardFailed("there is no primary selection".to_owned()))
    }
}
