//! A program's use of the clipboard (OSC 52, PLAN §6 and §9), as the pane keeps it: the question
//! waiting for the person. What a question means and how it is answered is the window's.

use temor_vt::ClipboardLocation;

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

/// The question and, for a copy, the text it would put on the clipboard, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingClipboard {
    pub ask: ClipboardAsk,
    pub text: String,
    pub to: ClipboardLocation,
}
