//! Tab in temor's composer: the shell completes the composer's line with its own completion, so
//! a command's completions are the ones the person's shell knows. temor types the line into the
//! shell's own (hidden) line and presses [`REQUEST_KEY`]; the integration script binds that key to
//! a function that answers in one OSC and empties its line again, so nothing of it is left to run.
//!
//! ```text
//! ESC ] 7504 ; v1 ; ready ST                       the shell answers the key (sent at each prompt)
//! ESC ] 7504 ; v1 ; at=N { ; c=WORD } ST           the line's characters from N on, completed
//! ```
//!
//! `N` counts characters of the line as it was typed. Each WORD is percent-encoded and takes the
//! place of the line's end from `N` on; a directory's ends in `/`. A tag this version does not
//! know is skipped, so a newer script can say more than an older temor reads.

use crate::osc7::percent_decode_strict;
use crate::suggest::Suggestion;

/// The OSC number the answers travel in.
pub const OSC_NUMBER: &str = "7504";

/// The key temor presses to ask: U+E000, a private-use character no keyboard types. Not an
/// escape sequence: fish 4 drops a sequence its key reader does not know before any binding
/// sees it, and passes a character on.
pub const REQUEST_KEY: &[u8] = "\u{E000}".as_bytes();

/// The most bytes of an answer that are read: a longer one is refused whole.
pub const MAX_ANSWER_BYTES: usize = 64 * 1024;

const VERSION: &str = "v1";

/// What the shell said.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Answer {
    /// The shell answers [`REQUEST_KEY`].
    Ready,
    /// The completions of the line asked about.
    Completed(Completions),
}

/// What the line's end can become: each word takes the place of the characters from `at` on.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Completions {
    pub at: usize,
    pub words: Vec<String>,
}

/// Why an answer was not read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AnswerFault {
    /// Longer than [`MAX_ANSWER_BYTES`].
    TooLarge,
    /// Not a version this build reads.
    Version,
    /// Neither `ready` nor `at=N` with valid words.
    Malformed,
}

/// The answer in a payload (the text after `ESC ] 7504 ;`).
pub fn decode(payload: &str) -> Result<Answer, AnswerFault> {
    if payload.len() > MAX_ANSWER_BYTES {
        return Err(AnswerFault::TooLarge);
    }
    let mut parts = payload.split(';');
    if parts.next() != Some(VERSION) {
        return Err(AnswerFault::Version);
    }
    let at = match parts.next() {
        Some("ready") => return Ok(Answer::Ready),
        Some(first) => first
            .strip_prefix("at=")
            .and_then(|n| n.parse().ok())
            .ok_or(AnswerFault::Malformed)?,
        None => return Err(AnswerFault::Malformed),
    };
    let mut words: Vec<String> = Vec::new();
    for part in parts.filter(|part| !part.is_empty()) {
        let Some(word) = part.strip_prefix("c=") else {
            // A tag a newer script sends: skipped, not an error.
            continue;
        };
        let word = percent_decode_strict(word)
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .ok_or(AnswerFault::Malformed)?;
        if !word.is_empty() && !words.contains(&word) {
            words.push(word);
        }
    }
    Ok(Answer::Completed(Completions { at, words }))
}

impl Completions {
    /// `line` with its end from `at` on made `word`: `None` when `at` is past the line.
    pub fn applied(&self, line: &str, word: &str) -> Option<String> {
        let start = line
            .char_indices()
            .map(|(i, _)| i)
            .chain([line.len()])
            .nth(self.at)?;
        Some(format!("{}{word}", &line[..start]))
    }

    /// The words as suggestions for `line`: each takes the place of the characters from `at` to
    /// the line's end, and a space follows unless it names a folder.
    pub fn suggestions(&self, line: &str) -> Vec<Suggestion> {
        let to = line.chars().count();
        self.words
            .iter()
            .map(|word| Suggestion {
                word: word.clone(),
                from: self.at,
                to,
                space: !word.ends_with('/'),
                detail: None,
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "complete_tests.rs"]
mod tests;
