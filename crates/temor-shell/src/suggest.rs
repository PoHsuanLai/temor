//! What completes the composer's line, whoever it is: a shell's own completion, nushell's, a model.
//! A [`CompletionSource`] is asked about a line and answers with [`Suggestion`]s, once, from any
//! thread, as late as it likes; the composer merges what the sources say into one list, and drops
//! an answer for a line that has changed since.
//!
//! A suggestion replaces the line's characters `from..to` with its word, so a source can finish
//! the last word (what a shell does) or put a whole command in the place of what was typed (what a
//! model does). This crate holds the vocabulary and the pure parts; the sources that run a process
//! or call a service live in the crates that may.

use std::path::PathBuf;
use std::sync::Arc;

/// Names a source in an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceId(pub &'static str);

/// The line to complete: its end is where the caret is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ask {
    pub line: String,
    /// The pane's folder, where the shell is.
    pub folder: Option<PathBuf>,
}

/// One way the line can go on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// What takes the place of the line's characters `from..to`.
    pub word: String,
    pub from: usize,
    pub to: usize,
    /// Whether a space follows when it is taken (a word does; a folder, which goes on, does not).
    pub space: bool,
    /// A few words about it, shown beside it.
    pub detail: Option<String>,
}

/// What a source said, for the request that asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answered {
    pub request: u64,
    pub source: SourceId,
    pub suggestions: Vec<Suggestion>,
}

/// How a source hands its answer back: callable from any thread.
pub type Deliver = Arc<dyn Fn(Answered) + Send + Sync>;

/// A source of suggestions that stands on its own: it needs nothing of the pane but the line and
/// the folder. (The running shell's own completion needs the pane, so the session holds it.)
pub trait CompletionSource: Send + Sync {
    /// Names the source in its answers.
    fn id(&self) -> SourceId;

    /// Whether it can answer now.
    fn ready(&self) -> bool {
        true
    }

    /// Starts answering. `deliver` is called once, now or later, from any thread; a source that
    /// has nothing to say says so with no suggestions.
    fn ask(&self, ask: Ask, request: u64, deliver: Deliver);
}

impl Suggestion {
    /// `line` with this suggestion put in: its characters `from..to` become the word, and a space
    /// follows where `space` says so. `None` when the place is not in the line.
    pub fn applied(&self, line: &str) -> Option<String> {
        let chars = line.chars().count();
        if self.from > self.to || self.to > chars {
            return None;
        }
        let byte = |at: usize| {
            line.char_indices()
                .map(|(i, _)| i)
                .chain([line.len()])
                .nth(at)
        };
        let (start, end) = (byte(self.from)?, byte(self.to)?);
        let mut out = format!("{}{}", &line[..start], self.word);
        if self.space {
            out.push(' ');
        }
        out.push_str(&line[end..]);
        Some(out)
    }
}

/// What every word starts with, on whole characters.
pub fn common_start<'a>(words: &[&'a str]) -> &'a str {
    let Some((first, rest)) = words.split_first() else {
        return "";
    };
    let mut end = first.len();
    for word in rest {
        end = first
            .char_indices()
            .zip(word.chars())
            .find(|((_, a), b)| a != b)
            .map_or(end.min(word.len()), |((i, _), _)| i.min(end));
    }
    &first[..end]
}

/// Where the line's last word starts, in characters: after the last space that is not inside
/// quotes (`"`, `'` or a backtick), so a quoted word with a space in it is one word.
pub fn last_word_start(line: &str) -> usize {
    let mut quote: Option<char> = None;
    let mut start = 0;
    for (i, c) in line.chars().enumerate() {
        match (quote, c) {
            (Some(open), c) if c == open => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'' | '`') => quote = Some(c),
            (None, c) if c.is_whitespace() => start = i + 1,
            (None, _) => {}
        }
    }
    start
}

/// Why nushell's answer was not read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum NushellFault {
    /// Not the JSON `nu --ide-complete` prints.
    NotJson,
}

/// The suggestions in what `nu --ide-complete` printed for `line`: each completion takes the
/// place of the last word. A completion nushell quotes (a name with a space) is put in as it is
/// quoted, and one that names a folder goes on, with no space after it.
pub fn read_nushell(output: &str, line: &str) -> Result<Vec<Suggestion>, NushellFault> {
    let value: serde_json::Value =
        serde_json::from_str(output).map_err(|_| NushellFault::NotJson)?;
    let list = value
        .get("completions")
        .and_then(serde_json::Value::as_array)
        .ok_or(NushellFault::NotJson)?;
    let (from, to) = (last_word_start(line), line.chars().count());
    let mut out: Vec<Suggestion> = Vec::new();
    for word in list.iter().filter_map(serde_json::Value::as_str) {
        let folder = word.trim_end_matches(['`', '"', '\'']).ends_with('/');
        let suggestion = Suggestion {
            word: word.to_owned(),
            from,
            to,
            space: !folder,
            detail: None,
        };
        if !word.is_empty() && !out.contains(&suggestion) {
            out.push(suggestion);
        }
    }
    Ok(out)
}

#[cfg(test)]
#[path = "suggest_tests.rs"]
mod tests;
