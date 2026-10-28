//! What a block's card says besides the command: one line of its output and how much output
//! there was.

/// The longest line of output a card keeps, in characters.
const LONGEST_LINE: usize = 160;

/// Words that make a line the one worth showing when a command failed.
const TROUBLE: [&str; 5] = ["error", "fatal", "failed", "panicked", "cannot"];

/// The line of output a card shows, and how much output there was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub text: String,
    /// How many lines the output has (blank ones included).
    pub lines: u32,
}

/// The note for `output`: when the command failed, the first line that names the trouble, else
/// the last line; for a command that did not fail, the last line (builds, installs and tests
/// say what they did at the end). Nothing when there was no output.
pub fn summarize(output: &str, failed: bool) -> Option<Note> {
    let lines = u32::try_from(output.lines().count()).unwrap_or(u32::MAX);
    let filled = || {
        output
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
    };
    let trouble = failed
        .then(|| {
            filled().find(|line| {
                let lower = line.to_lowercase();
                TROUBLE.iter().any(|word| lower.contains(word))
            })
        })
        .flatten();
    let line = trouble.or_else(|| filled().next_back())?;
    Some(Note {
        text: line.chars().take(LONGEST_LINE).collect(),
        lines,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failure_shows_the_line_that_names_the_trouble() {
        let out = "   Compiling a\nerror[E0425]: cannot find value `seed`\n --> src/x.rs:1\n\nerror: could not compile";
        let note = summarize(out, true).expect("a note");
        assert_eq!(note.text, "error[E0425]: cannot find value `seed`");
        assert_eq!(note.lines, 5);
    }

    #[test]
    fn a_failure_with_no_trouble_word_shows_the_last_line() {
        let note = summarize("one\ntwo\n\n", true).expect("a note");
        assert_eq!(note.text, "two");
    }

    #[test]
    fn a_command_that_did_not_fail_shows_its_last_line_even_if_it_says_error() {
        let note = summarize("error: none\nadded 412 packages\n", false).expect("a note");
        assert_eq!(note.text, "added 412 packages");
    }

    #[test]
    fn no_output_is_no_note() {
        assert_eq!(summarize("", false), None);
        assert_eq!(summarize("  \n\n", true), None);
    }

    #[test]
    fn a_long_line_is_cut() {
        let note = summarize(&"x".repeat(400), false).expect("a note");
        assert_eq!(note.text.chars().count(), LONGEST_LINE);
    }
}
