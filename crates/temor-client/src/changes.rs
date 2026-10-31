//! Comparing a host's file with the person's copy of it (PLAN §9b), for an edit conflict's
//! "Show changes": the plain line diff, theirs against yours, written beside the fetched copy
//! as a unified diff the viewer pane shows like any text file. The files are read, compared
//! and written on the copy's own thread ([`Compare`]), never the window's.

use std::path::PathBuf;

use similar::TextDiff;

use crate::viewer_host::{CopyDone, CopyOutcome};

/// The most either side may be and still be compared: a diff of more is not a thing to read.
const MOST_BYTES: usize = 2 * 1024 * 1024;
/// Lines of context around each change.
const CONTEXT: usize = 3;

/// What comparing the two sides found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Changes {
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
pub fn changes(theirs: &[u8], yours: &[u8], names: (&str, &str)) -> Changes {
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

/// What comparing a fetched file with the person's copy came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compared {
    /// The differences are written to the diff file, and the fetched copy is gone.
    Written,
    /// The two read the same.
    Same,
    /// One side is not text.
    NotText,
    /// One side is over the most compared.
    TooBig,
    /// The differences could not be written.
    Unwritten,
}

/// A copy's report, compared with `yours` first when the copy is whole: the report is then
/// [`CopyOutcome::Compared`].
pub struct Compare {
    /// The fetched copy of the host's file.
    pub theirs: PathBuf,
    /// The person's copy.
    pub yours: PathBuf,
    /// Where the differences are written.
    pub diff: PathBuf,
    /// The two sides' names, over the differences.
    pub names: (String, String),
    pub then: Box<dyn CopyDone>,
}

impl std::fmt::Debug for Compare {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Compare")
            .field("theirs", &self.theirs)
            .field("yours", &self.yours)
            .field("diff", &self.diff)
            .finish_non_exhaustive()
    }
}

impl CopyDone for Compare {
    fn done(self: Box<Self>, outcome: CopyOutcome) {
        let outcome = match outcome {
            CopyOutcome::Whole => CopyOutcome::Compared(self.compare()),
            other => other,
        };
        self.then.done(outcome);
    }
}

impl Compare {
    fn compare(&self) -> Compared {
        let found = match (std::fs::read(&self.theirs), std::fs::read(&self.yours)) {
            (Ok(theirs), Ok(yours)) => changes(&theirs, &yours, (&self.names.0, &self.names.1)),
            _ => Changes::NotText,
        };
        let text = match found {
            Changes::Text(text) => text,
            Changes::Same => return Compared::Same,
            Changes::NotText => return Compared::NotText,
            Changes::TooBig => return Compared::TooBig,
        };
        let written = std::fs::write(&self.diff, text);
        // The host's file served its turn; the differences are what is shown.
        let _ = std::fs::remove_file(&self.theirs);
        match written {
            Ok(()) => Compared::Written,
            Err(_) => {
                let _ = std::fs::remove_file(&self.diff);
                Compared::Unwritten
            }
        }
    }
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

    struct Heard(std::sync::mpsc::Sender<CopyOutcome>);

    impl CopyDone for Heard {
        fn done(self: Box<Self>, outcome: CopyOutcome) {
            let _ = self.0.send(outcome);
        }
    }

    fn compare(
        theirs: &[u8],
        yours: &[u8],
        outcome: CopyOutcome,
    ) -> (CopyOutcome, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("a folder");
        std::fs::write(dir.path().join("theirs"), theirs).expect("theirs");
        std::fs::write(dir.path().join("yours"), yours).expect("yours");
        let (tell, heard) = std::sync::mpsc::channel();
        Box::new(Compare {
            theirs: dir.path().join("theirs"),
            yours: dir.path().join("yours"),
            diff: dir.path().join("theirs.diff"),
            names: ("on the computer".to_owned(), "your copy".to_owned()),
            then: Box::new(Heard(tell)),
        })
        .done(outcome);
        (heard.recv().expect("told"), dir)
    }

    #[test]
    fn a_whole_copy_is_compared_and_its_differences_written_in_its_place() {
        let (outcome, dir) = compare(b"a\n", b"b\n", CopyOutcome::Whole);
        assert_eq!(outcome, CopyOutcome::Compared(Compared::Written));
        let diff = std::fs::read_to_string(dir.path().join("theirs.diff")).expect("the diff");
        assert!(diff.contains("+b"), "{diff}");
        assert!(!dir.path().join("theirs").exists());

        let (outcome, _dir) = compare(b"a\n", b"a\n", CopyOutcome::Whole);
        assert_eq!(outcome, CopyOutcome::Compared(Compared::Same));
        let (outcome, _dir) = compare(b"a\n", b"b\n", CopyOutcome::Gone);
        assert_eq!(outcome, CopyOutcome::Gone);
    }
}
