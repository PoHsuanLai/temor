//! Making a model's answer safe to type into a prompt (§8.3 step 1): one line, typed by
//! bracketed paste, so nothing in it may end the line, end the paste, drive the terminal or hide
//! from the person reading it.
//!
//! Removed, and reported by class: line breaks of every kind; tabs; whole escape sequences
//! (CSI, OSC and the other string sequences, two-character escapes), in both their 7-bit and
//! 8-bit forms, so `ESC [ 201 ~` cannot close the paste early and leaves no `[201~` behind;
//! other C0 and C1 controls and DEL; bidirectional controls, which can make the shown text
//! differ from what runs; and every Default_Ignorable_Code_Point (zero-width characters, fillers,
//! tags, variation selectors), which can carry text the person never sees. Look-alike spaces
//! (General_Category Zs other than U+0020) become a plain space, so what the person reads as two
//! words is two words to the shell. The Unicode tables are generated (`ucd_tables.rs`).

use super::ucd_tables::{DEFAULT_IGNORABLE, SPACE_SEPARATORS};
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
    /// A space that is not U+0020, replaced by one.
    Space,
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

/// `raw` with every unsafe character removed (or a look-alike space made plain), and a report of
/// what was.
pub fn sanitise(raw: &str) -> (SanitisedLine, Sanitised) {
    let mut kept = String::with_capacity(raw.len());
    let mut removed: Vec<CharClass> = Vec::new();
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        let fate = match c {
            '\u{1b}' => {
                skip_escape(&mut chars);
                Fate::Remove(CharClass::EscapeSequence)
            }
            '\u{9b}' => {
                skip_csi_body(&mut chars);
                Fate::Remove(CharClass::EscapeSequence)
            }
            '\u{90}' | '\u{98}' | '\u{9d}' | '\u{9e}' | '\u{9f}' => {
                skip_string(&mut chars);
                Fate::Remove(CharClass::EscapeSequence)
            }
            other => fate_of(other),
        };
        let class = match fate {
            Fate::Keep => {
                kept.push(c);
                continue;
            }
            Fate::Remove(class) => class,
            Fate::Plain(class) => {
                kept.push(' ');
                class
            }
        };
        if !removed.contains(&class) {
            removed.push(class);
        }
    }
    let report = if removed.is_empty() {
        Sanitised::Unchanged
    } else {
        Sanitised::Changed { removed }
    };
    (SanitisedLine(kept), report)
}

/// What happens to one character outside any sequence.
enum Fate {
    Keep,
    Remove(CharClass),
    /// Replaced by U+0020.
    Plain(CharClass),
}

/// The fate of a character outside any sequence. Bidi controls are default-ignorable too, and
/// are named apart because they are the ones that reorder what is shown.
fn fate_of(c: char) -> Fate {
    match c {
        '\n' | '\r' | '\u{0b}' | '\u{0c}' | '\u{85}' | '\u{2028}' | '\u{2029}' => {
            Fate::Remove(CharClass::Newline)
        }
        '\t' => Fate::Remove(CharClass::Tab),
        '\u{00}'..='\u{1f}' | '\u{7f}'..='\u{9f}' => Fate::Remove(CharClass::Control),
        '\u{061c}'
        | '\u{200e}'
        | '\u{200f}'
        | '\u{202a}'..='\u{202e}'
        | '\u{2066}'..='\u{2069}' => Fate::Remove(CharClass::Bidi),
        // Interlinear annotation marks hide the text between them, and are not default-ignorable.
        '\u{fff9}'..='\u{fffb}' => Fate::Remove(CharClass::Invisible),
        ' ' => Fate::Keep,
        c if within(c, DEFAULT_IGNORABLE) => Fate::Remove(CharClass::Invisible),
        c if within(c, SPACE_SEPARATORS) => Fate::Plain(CharClass::Space),
        _ => Fate::Keep,
    }
}

fn within(c: char, ranges: &[(u32, u32)]) -> bool {
    let code = u32::from(c);
    ranges
        .iter()
        .any(|&(first, last)| (first..=last).contains(&code))
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
