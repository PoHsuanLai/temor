// The cleaning table in `encode` is taken from Ghostty's `src/input/paste.zig`:
// Copyright (c) 2024 Mitchell Hashimoto, Ghostty contributors. MIT licence (see NOTICE).
//! Paste rules (PLAN §6): what makes pasted text need the person's confirmation, and how the bytes
//! are framed for the program. Pure: libghostty's own check arrives as a [`VtVerdict`] from the
//! app edge. The byte cleaning in [`encode`] follows Ghostty's `input/paste.zig` (MIT, see NOTICE).

use ds_core::word::Word;

const START_FRAME: &str = "\x1b[200~";
const END_FRAME: &str = "\x1b[201~";

/// Whether the program turned on bracketed paste (mode 2004).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
#[word(case = snake)]
pub enum PasteMode {
    Bracketed,
    Plain,
}

/// Why pasted text needs the person's confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
#[word(case = snake)]
pub enum UnsafeReason {
    /// A newline in a paste that is not framed: the shell would run it.
    Newline,
    /// The text contains the end of a bracketed paste, so it could leave the frame.
    EndOfFrame,
}

/// What libghostty's own check (`temor_vt::paste_is_safe`) said about the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VtVerdict {
    Clean,
    Flagged,
}

/// The verdict on a paste.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PasteCheck {
    Safe,
    NeedsConfirmation(UnsafeReason),
}

/// Checks text for `mode`. A carriage return is a line break for the shell, in either mode, so
/// it is always confirmed (libghostty does not check it). A newline is confirmed when the paste
/// is not bracketed. Whatever libghostty flagged (the end of a frame) is confirmed in every mode.
pub fn check(text: &str, mode: PasteMode, vt: VtVerdict) -> PasteCheck {
    let line_break = text.contains('\r') || (mode == PasteMode::Plain && text.contains('\n'));
    if line_break {
        return PasteCheck::NeedsConfirmation(UnsafeReason::Newline);
    }
    match vt {
        VtVerdict::Flagged => PasteCheck::NeedsConfirmation(UnsafeReason::EndOfFrame),
        VtVerdict::Clean => PasteCheck::Safe,
    }
}

/// The bytes to write to the program for `text`: control bytes that could drive the line
/// discipline become spaces (xterm's list, as Ghostty does); a bracketed paste is framed; a plain
/// paste turns newlines into carriage returns.
pub fn encode(text: &str, mode: PasteMode) -> Vec<u8> {
    let cleaned: String = text.chars().map(strip).collect();
    match mode {
        PasteMode::Bracketed => {
            let mut out = Vec::with_capacity(cleaned.len() + 12);
            out.extend_from_slice(START_FRAME.as_bytes());
            out.extend_from_slice(cleaned.as_bytes());
            out.extend_from_slice(END_FRAME.as_bytes());
            out
        }
        PasteMode::Plain => cleaned.replace('\n', "\r").into_bytes(),
    }
}

/// Control characters that become a space on paste: NUL, BS, ENQ, EOT, ESC, DEL, and the
/// terminal-driver keys Ghostty strips (Ctrl+C, Ctrl+\, Ctrl+U, Ctrl+Z, Ctrl+Q, Ctrl+S,
/// Ctrl+W, Ctrl+V, Ctrl+R).
fn strip(c: char) -> char {
    match c {
        '\0' | '\u{08}' | '\u{05}' | '\u{04}' | '\u{1b}' | '\u{7f}' | '\u{03}' | '\u{1c}'
        | '\u{15}' | '\u{1a}' | '\u{11}' | '\u{13}' | '\u{17}' | '\u{16}' | '\u{12}' => ' ',
        other => other,
    }
}
