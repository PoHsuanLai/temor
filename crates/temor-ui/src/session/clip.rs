//! Copy and paste (PLAN §6): copy writes the selection's text; paste frames the clipboard for
//! the program, and asks the person first when the text is unsafe to paste as it is.

use temor_pane::paste::{PasteCheck, PasteMode, UnsafeReason, VtVerdict, check, encode};

use super::Session;
use crate::error::UiError;

/// The largest paste the pane sends (8 MiB). Larger ones are refused, not truncated.
pub const PASTE_LIMIT: usize = 8 * 1024 * 1024;

/// A paste that needs the person's confirmation, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingPaste {
    pub text: String,
    pub reason: UnsafeReason,
}

impl Session {
    /// Copies the selected text to the clipboard, else the picked blocks whole (each command and
    /// its output). Nothing selected or picked: nothing happens.
    pub fn copy(&mut self) -> Result<(), UiError> {
        let text = match self.selection {
            Some(selection) => self
                .pane
                .with_term(|term| selection.text(|row| term.row_text(row).unwrap_or_default())),
            None => self.picked_text().unwrap_or_default(),
        };
        if text.is_empty() {
            return Ok(());
        }
        // A clipboard that refuses is not the pane's failure: the copy simply did not happen.
        if self.clipboard.write(&text).is_ok() {
            self.refresh_clipboard_copy();
        }
        Ok(())
    }

    /// Pastes the clipboard. An unsafe paste waits for [`Session::confirm_paste`]. A paste over
    /// [`PASTE_LIMIT`] is refused with a notice: it is never cut short.
    pub fn paste(&mut self) -> Result<(), UiError> {
        self.wake_cursor();
        let Ok(text) = self.clipboard.read() else {
            return Ok(());
        };
        if text.is_empty() {
            return Ok(());
        }
        if text.len() > PASTE_LIMIT {
            self.notice = Some(format!(
                "That paste is over {} MiB, so it was not pasted. Copy a smaller part.",
                PASTE_LIMIT / (1024 * 1024)
            ));
            return Ok(());
        }
        let mode = self.pane.with_term(|term| term.paste_mode());
        match check(&text, mode, vt_verdict(&text, mode)) {
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

/// libghostty's check, for the framing the program will get. A bracketed paste may hold line
/// breaks, so only the end-of-frame marker counts there: libghostty's check on the text with its
/// newlines turned to spaces is exactly that rule.
fn vt_verdict(text: &str, mode: PasteMode) -> VtVerdict {
    let safe = match mode {
        PasteMode::Bracketed => temor_vt::paste_is_safe(text.replace('\n', " ").as_bytes()),
        PasteMode::Plain => temor_vt::paste_is_safe(text.as_bytes()),
    };
    match safe {
        true => VtVerdict::Clean,
        false => VtVerdict::Flagged,
    }
}
