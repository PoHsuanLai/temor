//! Making a model's answer safe to type into a prompt (§8.3 step 1): one line, typed by
//! bracketed paste, so nothing in it may end the line, end the paste, drive the terminal or hide
//! from the person reading it.
//!
//! Removed, and reported by class: line breaks of every kind; tabs; whole escape sequences
//! (CSI, OSC and the other string sequences, two-character escapes), in both their 7-bit and
//! 8-bit forms, so `ESC [ 201 ~` cannot close the paste early and leaves no `[201~` behind;
//! other C0 and C1 controls and DEL; bidirectional controls, which can make the shown text
//! differ from what runs; and invisible characters (zero-width, tags, variation selectors), which
//! can carry text the person never sees.

use ds_core::word::Word;
use serde::{Deserialize, Serialize};

/// A kind of character the sanitiser removes.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Word, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum CharClass {
    /// A line break: LF, CR, VT, FF, NEL, U+2028, U+2029.
    Newline,
    /// A tab.
    Tab,
    /// A whole terminal escape sequence.
    EscapeSequence,
    /// Another control character: C0, DEL or C1.
    Control,
    /// A bidirectional formatting control.
    Bidi,
    /// A character that draws nothing.
    Invisible,
}

/// Whether sanitising changed the text, and what it took out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum Sanitised {
    /// Characters were removed; each class once, in the order first met.
    Changed {
        /// What was removed.
        removed: Vec<CharClass>,
    },
    /// The text was already clean.
    Unchanged,
}

/// One line of text with none of the classes above: what may be typed into a prompt. Reading
/// one from storage sanitises it again.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub struct SanitisedLine(String);

impl SanitisedLine {
    /// The text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether nothing is left.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<String> for SanitisedLine {
    fn from(text: String) -> SanitisedLine {
        sanitise(&text).0
    }
}

impl From<SanitisedLine> for String {
    fn from(line: SanitisedLine) -> String {
        line.0
    }
}

/// `raw` with every unsafe character removed, and a report of what was.
pub fn sanitise(raw: &str) -> (SanitisedLine, Sanitised) {
    let mut kept = String::with_capacity(raw.len());
    let mut removed: Vec<CharClass> = Vec::new();
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        let class = match c {
            '\u{1b}' => {
                skip_escape(&mut chars);
                Some(CharClass::EscapeSequence)
            }
            '\u{9b}' => {
                skip_csi_body(&mut chars);
                Some(CharClass::EscapeSequence)
            }
            '\u{90}' | '\u{98}' | '\u{9d}' | '\u{9e}' | '\u{9f}' => {
                skip_string(&mut chars);
                Some(CharClass::EscapeSequence)
            }
            other => class_of(other),
        };
        match class {
            Some(class) if !removed.contains(&class) => removed.push(class),
            Some(_) => {}
            None => kept.push(c),
        }
    }
    let report = if removed.is_empty() {
        Sanitised::Unchanged
    } else {
        Sanitised::Changed { removed }
    };
    (SanitisedLine(kept), report)
}

/// The class of a character outside any sequence, or none for one that is kept.
fn class_of(c: char) -> Option<CharClass> {
    match c {
        '\n' | '\r' | '\u{0b}' | '\u{0c}' | '\u{85}' | '\u{2028}' | '\u{2029}' => {
            Some(CharClass::Newline)
        }
        '\t' => Some(CharClass::Tab),
        '\u{00}'..='\u{1f}' | '\u{7f}'..='\u{9f}' => Some(CharClass::Control),
        '\u{061c}'
        | '\u{200e}'
        | '\u{200f}'
        | '\u{202a}'..='\u{202e}'
        | '\u{2066}'..='\u{2069}' => Some(CharClass::Bidi),
        '\u{00ad}'
        | '\u{180e}'
        | '\u{200b}'..='\u{200d}'
        | '\u{2060}'..='\u{2064}'
        | '\u{fe00}'..='\u{fe0f}'
        | '\u{feff}'
        | '\u{fff9}'..='\u{fffb}'
        | '\u{e0000}'..='\u{e007f}'
        | '\u{e0100}'..='\u{e01ef}' => Some(CharClass::Invisible),
        _ => None,
    }
}

type Chars<'a> = std::iter::Peekable<std::str::Chars<'a>>;

/// Skips what follows an ESC (ECMA-48 §5.3): a CSI, a string sequence, an escape with
/// intermediates, or one final character.
fn skip_escape(chars: &mut Chars<'_>) {
    match chars.peek().copied() {
        Some('[') => {
            chars.next();
            skip_csi_body(chars);
        }
        Some(']' | 'P' | 'X' | '^' | '_') => {
            chars.next();
            skip_string(chars);
        }
        Some('\u{20}'..='\u{2f}') => {
            while chars
                .next_if(|c| ('\u{20}'..='\u{2f}').contains(c))
                .is_some()
            {}
            chars.next_if(|c| ('\u{30}'..='\u{7e}').contains(c));
        }
        Some('\u{30}'..='\u{7e}') => {
            chars.next();
        }
        _ => {}
    }
}

/// Skips a CSI's parameters and intermediates, then its final byte if there is one.
fn skip_csi_body(chars: &mut Chars<'_>) {
    while chars
        .next_if(|c| ('\u{20}'..='\u{3f}').contains(c))
        .is_some()
    {}
    chars.next_if(|c| ('\u{40}'..='\u{7e}').contains(c));
}

/// Skips a string sequence's body up to and including its terminator (BEL, ESC \ or ST). An
/// unterminated string runs to the end, as a terminal would swallow it.
fn skip_string(chars: &mut Chars<'_>) {
    while let Some(c) = chars.next() {
        match c {
            '\u{07}' | '\u{9c}' => return,
            '\u{1b}' if chars.peek() == Some(&'\\') => {
                chars.next();
                return;
            }
            _ => {}
        }
    }
}
