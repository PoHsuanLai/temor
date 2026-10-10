//! A program's use of the clipboard (OSC 52, PLAN §6 and §9).
//!
//! Writes: a program in a pane on this computer may copy. A program on another computer asks
//! once per tab (the host's remembered answer, when the app has one, is set with
//! [`Session::set_clipboard_consent`]) and the answer is kept. Reads: every program asks once per
//! tab. libghostty wants a read answered at once, so the first request gets nothing; after the
//! person allows, the pane's copy of the clipboard (see `temor_pane::ClipboardMirror`) answers.

use temor_core::remote::Grant;
use temor_core::shelf::PaneSite;
use temor_pane::ReadPolicy;
use temor_vt::{ClipboardGrant, ClipboardRead, ClipboardWrite};

use super::Session;
use super::clip::PASTE_LIMIT;
use super::lifecycle::sanitise_title;
use super::model::Notice;

/// What to do with a program's clipboard write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WriteVerdict {
    Write,
    Ask,
    Drop,
}

/// Decides a write from where the pane's program runs, what the person has answered for it, and
/// whether the terminal already holds a grant for it.
pub(super) fn write_verdict(site: PaneSite, consent: Grant, held: ClipboardGrant) -> WriteVerdict {
    match (site, consent, held) {
        (PaneSite::Local, _, _) => WriteVerdict::Write,
        (_, Grant::Allowed, _) => WriteVerdict::Write,
        (_, Grant::Refused, _) => WriteVerdict::Drop,
        (_, Grant::Ask, ClipboardGrant::Granted) => WriteVerdict::Write,
        (_, Grant::Ask, _) => WriteVerdict::Ask,
    }
}

/// What the person is asked about the clipboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardUse {
    /// The program wants to put text on the clipboard.
    Copy,
    /// The program wants to read the clipboard.
    Read,
}

/// A question waiting for the person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardAsk {
    pub kind: ClipboardUse,
    /// The program's name, cleaned for showing; empty when it gave none.
    pub program: String,
}

/// The question and, for a copy, the text it would put on the clipboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PendingClipboard {
    pub(super) ask: ClipboardAsk,
    pub(super) text: String,
}

/// The text of a write: its first plain-text form, empty for a clear. `None` when the write
/// carries only other kinds of data.
fn write_text(write: &ClipboardWrite) -> Option<String> {
    if write.items.is_empty() {
        return Some(String::new());
    }
    let item = write.items.iter().find(|i| i.mime.starts_with("text/"))?;
    Some(String::from_utf8_lossy(&item.data).into_owned())
}

impl Session {
    /// Sets what the person has answered about this program's copying (a remembered answer for
    /// the host); `Grant::Ask` asks on the next write.
    pub fn set_clipboard_consent(&mut self, grant: Grant) {
        self.clip_write = grant;
    }

    /// The clipboard question waiting for an answer, if any.
    pub fn clipboard_ask(&self) -> Option<&ClipboardAsk> {
        self.clip_pending.as_ref().map(|p| &p.ask)
    }

    /// A program wrote to the clipboard.
    pub(super) fn osc52_write(&mut self, write: ClipboardWrite) -> Notice {
        let Some(text) = write_text(&write) else {
            return Notice::Quiet;
        };
        if text.len() > PASTE_LIMIT {
            self.notice =
                Some("A program tried to copy more than 8 MiB, so it was not copied.".into());
            return Notice::Quiet;
        }
        match write_verdict(self.site, self.clip_write, write.grant) {
            WriteVerdict::Write => self.put_on_clipboard(&text),
            WriteVerdict::Drop => {}
            WriteVerdict::Ask if self.clip_pending.is_some() => {}
            WriteVerdict::Ask => {
                let ask = ClipboardAsk {
                    kind: ClipboardUse::Copy,
                    program: sanitise_title(&write.program),
                };
                self.clip_pending = Some(PendingClipboard { ask, text });
                return Notice::Chrome;
            }
        }
        Notice::Quiet
    }

    /// A program asked to read the clipboard before the person was asked.
    pub(super) fn osc52_read_asked(&mut self, read: ClipboardRead) -> Notice {
        if self.clip_pending.is_some() {
            return Notice::Quiet;
        }
        let ask = ClipboardAsk {
            kind: ClipboardUse::Read,
            program: sanitise_title(&read.program),
        };
        self.clip_pending = Some(PendingClipboard {
            ask,
            text: String::new(),
        });
        Notice::Chrome
    }

    /// The person answered the waiting question. The answer is kept for this tab.
    pub fn answer_clipboard(&mut self, grant: Grant) {
        let Some(pending) = self.clip_pending.take() else {
            return;
        };
        match pending.ask.kind {
            ClipboardUse::Copy => {
                self.clip_write = grant;
                if grant == Grant::Allowed {
                    self.put_on_clipboard(&pending.text);
                }
            }
            ClipboardUse::Read => {
                let policy = match grant {
                    Grant::Allowed => ReadPolicy::Allowed,
                    Grant::Refused => ReadPolicy::Refused,
                    Grant::Ask => ReadPolicy::Ask,
                };
                self.clip_mirror.set_policy(policy);
                self.refresh_clipboard_copy();
            }
        }
    }

    /// Puts `text` on the desktop clipboard and keeps the pane's copy in step.
    fn put_on_clipboard(&mut self, text: &str) {
        // A clipboard that refuses is not the pane's failure: the copy simply did not happen.
        if self.clipboard.write(text).is_ok() {
            self.refresh_clipboard_copy();
        }
    }

    /// Re-reads the desktop clipboard into the pane's copy, when the person lets programs read it.
    pub(super) fn refresh_clipboard_copy(&self) {
        if self.clip_mirror.policy() != ReadPolicy::Allowed {
            return;
        }
        self.clip_mirror.set_text(self.clipboard.read().ok());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_write_is_decided_by_where_the_program_runs_and_what_was_answered() {
        use ClipboardGrant::{Ask as NoGrant, Granted};
        use PaneSite::{Local, Remote};
        use WriteVerdict::{Ask, Drop, Write};
        let table = [
            (Local, Grant::Ask, NoGrant, Write),
            (Local, Grant::Refused, NoGrant, Write),
            (Remote, Grant::Ask, NoGrant, Ask),
            (Remote, Grant::Ask, Granted, Write),
            (Remote, Grant::Allowed, NoGrant, Write),
            (Remote, Grant::Refused, Granted, Drop),
        ];
        for (site, consent, held, want) in table {
            assert_eq!(
                write_verdict(site, consent, held),
                want,
                "{site:?} {consent:?}"
            );
        }
    }
}
