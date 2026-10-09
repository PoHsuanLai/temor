//! Paste rules, after Ghostty's `input/paste.zig` (MIT): what makes pasted text unsafe, and how
//! the bytes are framed for the program. Pure; the app asks the person before an unsafe paste.

use ds_core::word::Word;

const END_FRAME: &str = "\x1b[201~";
const START_FRAME: &str = "\x1b[200~";

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

/// The verdict on a paste.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PasteCheck {
    Safe,
    NeedsConfirmation(UnsafeReason),
}

/// Checks text against the paste rules for `mode`. The end-of-frame marker is unsafe in every
/// mode, as in Ghostty; a newline is unsafe only when the paste is not bracketed.
pub fn check(text: &str, mode: PasteMode) -> PasteCheck {
    if text.contains(END_FRAME) {
        return PasteCheck::NeedsConfirmation(UnsafeReason::EndOfFrame);
    }
    match mode {
        PasteMode::Bracketed => PasteCheck::Safe,
        PasteMode::Plain if text.contains('\n') => {
            PasteCheck::NeedsConfirmation(UnsafeReason::Newline)
        }
        PasteMode::Plain => PasteCheck::Safe,
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
