//! What a block's card says besides the command: one line of its output, how much output there
//! was, how long it ran against the other cards, and how old it looks. All pure, so the card
//! view only places what these answer.

/// The longest line of output a card keeps, in characters.
const LONGEST_LINE: usize = 160;

/// Words that make a line the one worth showing when a command failed.
const TROUBLE: [&str; 5] = ["error", "fatal", "failed", "panicked", "cannot"];

/// How the line is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteTone {
    Plain,
    /// The line says what went wrong.
    Trouble,
}

/// The line of output a card shows, and how much output there was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub text: String,
    pub tone: NoteTone,
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
    let (line, tone) = match trouble {
        Some(line) => (line, NoteTone::Trouble),
        None => (filled().next_back()?, NoteTone::Plain),
    };
    Some(Note {
        text: line.chars().take(LONGEST_LINE).collect(),
        tone,
        lines,
    })
}

/// How many bars of a card's size glyph to draw, 1 to 4.
pub fn glyph_bars(lines: u32) -> u8 {
    match lines {
        0..=2 => 1,
        3..=8 => 2,
        9..=40 => 3,
        _ => 4,
    }
}

/// The part of the card's width its duration bar takes, 0 to 1, against the longest of the
/// cards in view. A command that took no time still draws a sliver.
pub fn bar_share(took_ms: u64, longest_ms: u64) -> f32 {
    const SLIVER: f32 = 0.02;
    if longest_ms == 0 {
        return SLIVER;
    }
    (took_ms as f32 / longest_ms as f32).clamp(SLIVER, 1.0)
}

/// How strongly a card draws: the newest are full, older ones fade down to a floor, in steps
/// from the newest (`rank` 0).
pub fn age_strength(rank: usize) -> f32 {
    const FLOOR: f32 = 0.6;
    const STEP: f32 = 0.06;
    (1.0 - STEP * rank as f32).max(FLOOR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failure_shows_the_line_that_names_the_trouble() {
        let out = "   Compiling a\nerror[E0425]: cannot find value `seed`\n --> src/x.rs:1\n\nerror: could not compile";
        let note = summarize(out, true).expect("a note");
        assert_eq!(note.text, "error[E0425]: cannot find value `seed`");
        assert_eq!(note.tone, NoteTone::Trouble);
        assert_eq!(note.lines, 5);
    }

    #[test]
    fn a_failure_with_no_trouble_word_shows_the_last_line() {
        let note = summarize("one\ntwo\n\n", true).expect("a note");
        assert_eq!((note.text.as_str(), note.tone), ("two", NoteTone::Plain));
    }

    #[test]
    fn a_command_that_did_not_fail_shows_its_last_line_even_if_it_says_error() {
        let note = summarize("error: none\nadded 412 packages\n", false).expect("a note");
        assert_eq!(note.text, "added 412 packages");
        assert_eq!(note.tone, NoteTone::Plain);
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

    #[test]
    fn the_glyph_grows_with_the_output() {
        let bars: Vec<u8> = [0, 2, 3, 8, 9, 40, 41].map(glyph_bars).to_vec();
        assert_eq!(bars, [1, 1, 2, 2, 3, 3, 4]);
    }

    #[test]
    fn the_bar_is_the_share_of_the_longest_with_a_sliver_as_the_least() {
        assert_eq!(bar_share(500, 1000), 0.5);
        assert_eq!(bar_share(1000, 1000), 1.0);
        assert_eq!(bar_share(0, 1000), 0.02);
        assert_eq!(bar_share(0, 0), 0.02);
        assert_eq!(bar_share(5000, 1000), 1.0);
    }

    #[test]
    fn older_cards_fade_to_a_floor() {
        assert_eq!(age_strength(0), 1.0);
        assert!(age_strength(1) < 1.0);
        assert_eq!(age_strength(50), 0.6);
    }
}
