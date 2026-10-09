//! Copy and paste (PLAN §6): copy writes the selection's text; paste frames the clipboard for
//! the program, and asks the person first when the text is unsafe to paste as it is.

use temor_core::terminal::paste::{PasteCheck, UnsafeReason, check, encode};

use super::Session;
use crate::error::UiError;

/// A paste that needs the person's confirmation, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingPaste {
    pub text: String,
    pub reason: UnsafeReason,
}

impl Session {
    /// Copies the selected text to the clipboard. Nothing is selected: nothing happens.
    pub fn copy(&mut self) -> Result<(), UiError> {
        let Some(selection) = self.selection else {
            return Ok(());
        };
        let text = self
            .pane
            .with_term(|term| selection.text(|row| term.row_text(row).unwrap_or_default()));
        if text.is_empty() {
            return Ok(());
        }
        // A clipboard that refuses is not the pane's failure: the copy simply did not happen.
        let _ = self.clipboard.write(&text);
        Ok(())
    }

    /// Pastes the clipboard. An unsafe paste waits for [`Session::confirm_paste`].
    pub fn paste(&mut self) -> Result<(), UiError> {
        let Ok(text) = self.clipboard.read() else {
            return Ok(());
        };
        if text.is_empty() {
            return Ok(());
        }
        let mode = self.pane.with_term(|term| term.paste_mode());
        match check(&text, mode) {
            PasteCheck::Safe => self.pane.send(&encode(&text, mode)),
            PasteCheck::NeedsConfirmation(reason) => {
                self.pending = Some(PendingPaste { text, reason })
            }
        }
        Ok(())
    }

    /// The person agreed to the waiting paste: it is sent as it is.
    pub fn confirm_paste(&mut self) {
        if let Some(pending) = self.pending.take() {
            let mode = self.pane.with_term(|term| term.paste_mode());
            self.pane.send(&encode(&pending.text, mode));
        }
    }

    /// The person declined the waiting paste.
    pub fn cancel_paste(&mut self) {
        self.pending = None;
    }
}
