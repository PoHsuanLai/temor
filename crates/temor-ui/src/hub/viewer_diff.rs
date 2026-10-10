//! The plain line diff an edit conflict shows (PLAN §9b): the host's file as it is now against
//! the person's copy, as a unified diff the viewer pane shows like any text file.

use similar::TextDiff;

/// The most either side may be and still be compared: a diff of more is not a thing to read.
const MOST_BYTES: usize = 2 * 1024 * 1024;
/// Lines of context around each change.
const CONTEXT: usize = 3;

/// What comparing the two sides found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Changes {
    /// The two read the same.
    Same,
    /// The unified diff, theirs against yours.
    Text(String),
    /// One side is not text.
    NotText,
    /// One side is over the most compared.
    TooBig,
}

/// The changes from `theirs` (the host's file now) to `yours`, headed with the two names.
pub(super) fn changes(theirs: &[u8], yours: &[u8], names: (&str, &str)) -> Changes {
    if theirs.len() > MOST_BYTES || yours.len() > MOST_BYTES {
        return Changes::TooBig;
    }
    let (Ok(theirs), Ok(yours)) = (std::str::from_utf8(theirs), std::str::from_utf8(yours)) else {
        return Changes::NotText;
    };
    if theirs == yours {
        return Changes::Same;
    }
    let diff = TextDiff::from_lines(theirs, yours);
    let text = diff
        .unified_diff()
        .context_radius(CONTEXT)
        .header(names.0, names.1)
        .to_string();
    Changes::Text(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_diff_shows_what_was_removed_and_added_and_refuses_what_it_cannot_read() {
        let names = ("on the computer", "your copy");
        let Changes::Text(text) = changes(b"a\nb\nc\n", b"a\nB\nc\nd\n", names) else {
            panic!("a diff");
        };
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "--- on the computer");
        assert_eq!(lines[1], "+++ your copy");
        for line in ["-b", "+B", "+d", " a"] {
            assert!(lines.contains(&line), "{line} in {lines:?}");
        }
        let table = [
            ("same", changes(b"x\n", b"x\n", names), Changes::Same),
            (
                "binary",
                changes(&[0xff, 0xfe], b"x", names),
                Changes::NotText,
            ),
            (
                "big",
                changes(&vec![b'a'; MOST_BYTES + 1], b"x", names),
                Changes::TooBig,
            ),
        ];
        for (why, got, want) in table {
            assert_eq!(got, want, "{why}");
        }
    }
}
