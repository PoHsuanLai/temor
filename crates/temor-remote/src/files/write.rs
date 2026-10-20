//! The write-back rule: what a write must find before it replaces a file.

use super::stamp::sha256;
use temor_wire::message::{FileStamp, WriteCheck};

/// What every staging file's name starts with.
pub const STAGE_PREFIX: &str = ".temor-part-";

/// The name of the hidden file a streamed write stages in, beside its target. Derived from the
/// target's name so each target has its own and a retry finds the same one.
pub fn stage_name(leaf: &str) -> String {
    format!("{STAGE_PREFIX}{}", sha256(leaf.as_bytes()).short())
}

/// What a write does, given its check and what is there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteDecision {
    /// Replace the file (or make it).
    Replace,
    /// Make the file, which must not exist.
    Create,
    /// Refuse: the file is not what the writer last saw.
    Conflict,
}

/// Decides a write from its [`WriteCheck`] and the stamp of the file now (`None` if absent).
pub fn write_decision(check: &WriteCheck, current: Option<FileStamp>) -> WriteDecision {
    match check {
        WriteCheck::Overwrite => WriteDecision::Replace,
        WriteCheck::Unchanged(seen) if current == Some(*seen) => WriteDecision::Replace,
        WriteCheck::MustBeNew if current.is_none() => WriteDecision::Create,
        _ => WriteDecision::Conflict,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use temor_wire::digest::ContentHash;
    use temor_wire::units::UnixMillis;

    fn stamp(mtime: i64, byte: u8) -> FileStamp {
        FileStamp {
            mtime: UnixMillis(mtime),
            hash: ContentHash([byte; 32]),
        }
    }

    #[test]
    fn each_target_stages_in_its_own_hidden_file() {
        assert_eq!(stage_name("a.txt"), stage_name("a.txt"));
        assert_ne!(stage_name("a.txt"), stage_name("b.txt"));
        assert!(stage_name("a.txt").starts_with(".temor-part-"));
        assert_eq!(stage_name("a.txt").len(), ".temor-part-".len() + 16);
    }

    #[test]
    fn a_write_needs_the_file_to_be_exactly_what_was_seen() {
        let seen = stamp(10, 1);
        let cases = [
            (WriteCheck::Overwrite, None, WriteDecision::Replace),
            (WriteCheck::Overwrite, Some(seen), WriteDecision::Replace),
            (
                WriteCheck::Unchanged(seen),
                Some(seen),
                WriteDecision::Replace,
            ),
            // Same time, other contents: the hash catches what the mtime misses.
            (
                WriteCheck::Unchanged(seen),
                Some(stamp(10, 2)),
                WriteDecision::Conflict,
            ),
            (
                WriteCheck::Unchanged(seen),
                Some(stamp(11, 1)),
                WriteDecision::Conflict,
            ),
            (WriteCheck::Unchanged(seen), None, WriteDecision::Conflict),
            (WriteCheck::MustBeNew, None, WriteDecision::Create),
            (WriteCheck::MustBeNew, Some(seen), WriteDecision::Conflict),
        ];
        for (check, current, want) in cases {
            assert_eq!(
                write_decision(&check, current),
                want,
                "{check:?} {current:?}"
            );
        }
    }
}
