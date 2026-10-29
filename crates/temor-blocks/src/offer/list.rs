//! The commands that get an offer: a fixed default list, the person's additions and removals,
//! and the match on a command line's first words.

use std::fmt;

/// The fixed list, each as the first words of a command line. A word starting with `-` may sit
/// anywhere after the leading words (`docker run --rm -it ubuntu` matches `docker run -it`).
pub const DEFAULT_COMMANDS: [&str; 13] = [
    "ssh",
    "mosh",
    "docker exec",
    "docker run -it",
    "podman exec",
    "kubectl exec",
    "sudo -i",
    "sudo -s",
    "su",
    "nix-shell",
    "nix develop",
    "poetry shell",
    "pipenv shell",
];

/// A listed command as the person sees it ("ssh", "docker exec"): what an offer is about, and
/// what "Never for" adds to the remove list.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Program(pub String);

impl fmt::Display for Program {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One listed command, split into the words that must lead and the words that may follow.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Pattern {
    text: String,
    lead: Vec<String>,
    later: Vec<String>,
}

impl Pattern {
    fn new(text: &str) -> Option<Self> {
        let words: Vec<String> = text.split_whitespace().map(str::to_owned).collect();
        let split = words
            .iter()
            .position(|w| w.starts_with('-'))
            .unwrap_or(words.len());
        if split == 0 {
            return None;
        }
        Some(Self {
            text: words.join(" "),
            lead: words[..split].to_vec(),
            later: words[split..].to_vec(),
        })
    }

    fn matches(&self, words: &[&str]) -> bool {
        let led = words.len() >= self.lead.len()
            && self
                .lead
                .iter()
                .zip(words)
                .all(|(want, got)| want.as_str() == *got);
        if !led {
            return false;
        }
        let after = &words[self.lead.len()..];
        self.later.iter().all(|want| match want.starts_with('-') {
            true => after.iter().any(|got| flag_matches(want, got)),
            false => after.contains(&want.as_str()),
        })
    }
}

/// A flag matches itself; a short cluster (`-it`) is also in any cluster holding its letters
/// (`-ti`, `-dit`).
fn flag_matches(want: &str, got: &str) -> bool {
    if want == got {
        return true;
    }
    let short = |text: &str| text.starts_with('-') && !text.starts_with("--") && text.len() > 1;
    short(want) && short(got) && want.chars().skip(1).all(|c| got[1..].contains(c))
}

/// The command's words for matching: leading `NAME=value` assignments dropped, the program
/// named by its file name (`/usr/bin/ssh` is `ssh`).
fn words_of(command: &str) -> Vec<&str> {
    let mut words: Vec<&str> = command
        .split_whitespace()
        .skip_while(|word| is_assignment(word))
        .collect();
    if let Some(first) = words.first_mut() {
        let name: &str = first;
        *first = name.rsplit('/').next().unwrap_or(name);
    }
    words
}

fn is_assignment(word: &str) -> bool {
    word.split_once('=').is_some_and(|(name, _)| {
        !name.is_empty()
            && !name.starts_with(|c: char| c.is_ascii_digit())
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

/// The commands that get an offer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfferList {
    patterns: Vec<Pattern>,
}

impl Default for OfferList {
    fn default() -> Self {
        Self::new(&[], &[])
    }
}

impl OfferList {
    /// The default list, then `add`, minus every entry whose text is in `remove`. Entries with
    /// no leading word (empty, or starting with a flag) are skipped.
    pub fn new(add: &[String], remove: &[String]) -> Self {
        let mut patterns: Vec<Pattern> = Vec::new();
        let texts = DEFAULT_COMMANDS
            .iter()
            .copied()
            .chain(add.iter().map(String::as_str));
        for text in texts {
            let Some(pattern) = Pattern::new(text) else {
                continue;
            };
            let removed = remove
                .iter()
                .any(|r| r.split_whitespace().eq(pattern.text.split(' ')));
            if !removed && !patterns.contains(&pattern) {
                patterns.push(pattern);
            }
        }
        Self { patterns }
    }

    /// The listed command `command` starts with, if any. Reads only the typed words.
    pub fn matches(&self, command: &str) -> Option<Program> {
        let words = words_of(command);
        self.patterns
            .iter()
            .find(|pattern| pattern.matches(&words))
            .map(|pattern| Program(pattern.text.clone()))
    }

    /// Takes `program` off the list.
    pub fn remove(&mut self, program: &Program) {
        self.patterns.retain(|pattern| pattern.text != program.0);
    }
}
