//! A process's command line, as `/proc/<pid>/cmdline` holds it.

use std::fmt;

/// The most arguments a kept command line has; more and it is not kept.
pub const MAX_TOKENS: usize = 256;

/// The most bytes of `cmdline` that are read; a longer one is not kept.
pub const MAX_BYTES: usize = 64 * 1024;

/// The arguments a process was started with, program first. Bounded, and never printed whole:
/// arguments can hold secrets, so `Debug` shows only how many there are.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct CommandLine(Vec<String>);

impl CommandLine {
    /// The command line in `bytes`, the contents of a `cmdline` file: arguments each ended by a
    /// NUL, read as lossy UTF-8. `None` when it is empty, longer than [`MAX_BYTES`], or has more
    /// than [`MAX_TOKENS`] arguments.
    pub fn from_cmdline(bytes: &[u8]) -> Option<Self> {
        if bytes.is_empty() || bytes.len() > MAX_BYTES {
            return None;
        }
        let body = bytes.strip_suffix(&[0]).unwrap_or(bytes);
        let tokens: Vec<String> = body
            .split(|b| *b == 0)
            .map(|token| String::from_utf8_lossy(token).into_owned())
            .collect();
        (tokens.len() <= MAX_TOKENS).then_some(Self(tokens))
    }

    /// The command line from the word at `at` on: the program a wrapper runs and its arguments,
    /// without the wrapper's own words (an `env` assignment can hold a secret). `None` when
    /// there is no word at `at`.
    pub fn from_program(&self, at: usize) -> Option<Self> {
        let rest = self.0.get(at..).filter(|rest| !rest.is_empty())?;
        Some(Self(rest.to_vec()))
    }

    /// The arguments, program first.
    pub fn tokens(&self) -> &[String] {
        &self.0
    }
}

impl fmt::Debug for CommandLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CommandLine({} tokens)", self.0.len())
    }
}
