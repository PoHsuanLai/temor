//! A strict, small dotenv reader. Pure: it takes the file's text and gives back variables; the
//! edge reads the file and decides what a fault means. Nothing is expanded, run or looked up.
//!
//! The rules, all of them:
//! - One entry per line. Blank lines and lines whose first non-blank character is `#` are
//!   skipped. A line break may be LF or CRLF, and a leading byte-order mark is ignored.
//! - An entry is `KEY=value`, optionally after `export ` (so the file also works with `source`).
//!   `KEY` is a letter or `_`, then letters, digits or `_`. There is no space before the `=`;
//!   a blank after it belongs to the value only when the value is quoted.
//! - **Unquoted** value: everything up to the end of the line or up to a `#` that follows a blank
//!   (`A=b # note` is `b`; `A=b#c` is `b#c`), with blanks trimmed at both ends. No escapes, and
//!   quote characters inside are ordinary.
//! - **Single-quoted** value: exactly the text between the quotes, with no escapes.
//! - **Double-quoted** value: the text between the quotes, where `\n`, `\r`, `\t`, `\\`, `\"` and
//!   `\$` mean a line feed, carriage return, tab, backslash, quote and dollar sign. Any other
//!   backslash is a fault.
//! - After a closing quote only blanks and a `#` comment may follow. A quote must close on its
//!   own line; there are no multi-line values.
//! - `$` is never special: `A=$HOME` is the text `$HOME`, and `${X}` and `$(cmd)` stay text.
//! - A control character other than tab inside a value is a fault.
//! - A key that appears twice keeps its first place and its last value.
//!
//! A bad line is reported with its number and skipped; the good lines still apply.

use temor_core::vocab::text::{EnvKey, EnvValue};

/// What is wrong with one line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DotenvFault {
    /// No `=` on the line.
    NoEquals,
    /// The name is empty or has a character a variable name may not have.
    BadKey,
    /// A quote that is never closed on its line.
    Unterminated,
    /// Text after a closing quote that is neither blank nor a comment.
    TrailingText,
    /// A backslash escape that is not one of the listed ones.
    BadEscape,
    /// A control character in the value.
    ControlCharacter,
}

impl DotenvFault {
    /// What to tell the person, without jargon.
    pub fn describe(self) -> &'static str {
        match self {
            DotenvFault::NoEquals => "expected NAME=value",
            DotenvFault::BadKey => "the name may only have letters, digits and _",
            DotenvFault::Unterminated => "a quote is not closed",
            DotenvFault::TrailingText => "unexpected text after the closing quote",
            DotenvFault::BadEscape => "unknown escape; use \\n, \\r, \\t, \\\\, \\\" or \\$",
            DotenvFault::ControlCharacter => "the value has a control character",
        }
    }
}

/// What reading a dotenv file gave.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Dotenv {
    /// The variables, in the order their names first appear.
    pub vars: Vec<(EnvKey, EnvValue)>,
    /// The lines that were skipped, by line number from 1.
    pub faults: Vec<(usize, DotenvFault)>,
}

/// The variables in `text`.
pub fn parse_dotenv(text: &str) -> Dotenv {
    let mut out = Dotenv::default();
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    for (index, line) in text.lines().enumerate() {
        match parse_line(line) {
            Ok(None) => {}
            Ok(Some((key, value))) => put(&mut out.vars, key, value),
            Err(fault) => out.faults.push((index + 1, fault)),
        }
    }
    out
}

fn put(vars: &mut Vec<(EnvKey, EnvValue)>, key: EnvKey, value: EnvValue) {
    match vars.iter_mut().find(|(k, _)| *k == key) {
        Some(slot) => slot.1 = value,
        None => vars.push((key, value)),
    }
}

fn is_blank(c: char) -> bool {
    c == ' ' || c == '\t'
}

fn parse_line(line: &str) -> Result<Option<(EnvKey, EnvValue)>, DotenvFault> {
    let line = line.trim_start_matches(is_blank);
    if line.is_empty() || line.starts_with('#') {
        return Ok(None);
    }
    let line = match line.strip_prefix("export") {
        Some(rest) if rest.starts_with(is_blank) => rest.trim_start_matches(is_blank),
        _ => line,
    };
    let (name, rest) = line.split_once('=').ok_or(DotenvFault::NoEquals)?;
    let mut chars = name.chars();
    let starts = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    if !starts || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(DotenvFault::BadKey);
    }
    let key = EnvKey::new(name).map_err(|_| DotenvFault::BadKey)?;
    // Checked on the text as written, so a `\n` escape may still make a line feed.
    if rest.chars().any(|c| c.is_control() && c != '\t') {
        return Err(DotenvFault::ControlCharacter);
    }
    let value = match rest.chars().next() {
        Some('\'') => single_quoted(&rest[1..])?,
        Some('"') => double_quoted(&rest[1..])?,
        _ => unquoted(rest),
    };
    Ok(Some((key, EnvValue(value))))
}

fn unquoted(rest: &str) -> String {
    let mut end = rest.len();
    let mut previous_blank = false;
    for (at, c) in rest.char_indices() {
        if c == '#' && previous_blank {
            end = at;
            break;
        }
        previous_blank = is_blank(c);
    }
    rest[..end].trim_matches(is_blank).to_owned()
}

/// Only blanks, or blanks and a comment, may follow a closing quote.
fn only_comment(after: &str) -> Result<(), DotenvFault> {
    let after = after.trim_start_matches(is_blank);
    if after.is_empty() || after.starts_with('#') {
        Ok(())
    } else {
        Err(DotenvFault::TrailingText)
    }
}

fn single_quoted(rest: &str) -> Result<String, DotenvFault> {
    let (value, after) = rest.split_once('\'').ok_or(DotenvFault::Unterminated)?;
    only_comment(after)?;
    Ok(value.to_owned())
}

fn double_quoted(rest: &str) -> Result<String, DotenvFault> {
    let mut value = String::new();
    let mut chars = rest.char_indices();
    while let Some((at, c)) = chars.next() {
        match c {
            '"' => {
                only_comment(&rest[at + 1..])?;
                return Ok(value);
            }
            '\\' => {
                value.push(match chars.next().map(|(_, e)| e) {
                    Some('n') => '\n',
                    Some('r') => '\r',
                    Some('t') => '\t',
                    Some(e @ ('\\' | '"' | '$')) => e,
                    _ => return Err(DotenvFault::BadEscape),
                });
            }
            c => value.push(c),
        }
    }
    Err(DotenvFault::Unterminated)
}
