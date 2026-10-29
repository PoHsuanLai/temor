//! Which way a remote edit reaches the host's file (PLAN §9b): through the helper when the host
//! has one running (Tier B), where the write itself checks that the file is still the one that
//! was read; with `scp` and a look at the file's time and size beforehand otherwise (Tier A).

use crate::model::host::master::Master;
use crate::remote::message::{FileStamp, WriteCheck};

/// How an edit's file is read and written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditTier {
    /// `scp` over the shared connection.
    Scp,
    /// The host's helper.
    Helper,
}

/// Whether the host's helper is connected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelperUp {
    Yes,
    No,
}

/// Whether the next write goes over a changed file because the person said so.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overwrite {
    Yes,
    No,
}

/// There is no way to reach the host's files: no helper, and no shared connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoWay;

/// The way to reach a host's files: the helper when it is connected, else `scp` on a shared
/// connection that is up.
pub fn pick_tier(helper: HelperUp, master: Option<&Master>) -> Result<EditTier, NoWay> {
    match (helper, master) {
        (HelperUp::Yes, _) => Ok(EditTier::Helper),
        (HelperUp::No, Some(Master::Up)) => Ok(EditTier::Scp),
        (HelperUp::No, _) => Err(NoWay),
    }
}

/// What a write-back must find on the host. The stamp read last, unless the person chose to
/// overwrite; `None` when there is no stamp to hold the write to (it is never written blind).
pub fn write_check(seen: Option<FileStamp>, overwrite: Overwrite) -> Option<WriteCheck> {
    match (overwrite, seen) {
        (Overwrite::Yes, _) => Some(WriteCheck::Overwrite),
        (Overwrite::No, Some(stamp)) => Some(WriteCheck::Unchanged(stamp)),
        (Overwrite::No, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote::digest::ContentHash;
    use crate::remote::units::UnixMillis;

    #[test]
    fn a_host_with_a_helper_uses_it_and_one_without_needs_its_shared_connection() {
        let cases = [
            (
                "helper, no shared connection",
                HelperUp::Yes,
                None,
                Ok(EditTier::Helper),
            ),
            (
                "helper and shared connection",
                HelperUp::Yes,
                Some(Master::Up),
                Ok(EditTier::Helper),
            ),
            (
                "no helper, connection up",
                HelperUp::No,
                Some(Master::Up),
                Ok(EditTier::Scp),
            ),
            ("no helper, no connection", HelperUp::No, None, Err(NoWay)),
            (
                "no helper, connection not up",
                HelperUp::No,
                Some(Master::Down),
                Err(NoWay),
            ),
        ];
        for (why, helper, master, want) in cases {
            assert_eq!(pick_tier(helper, master.as_ref()), want, "{why}");
        }
    }

    #[test]
    fn a_write_is_held_to_what_was_read_unless_the_person_chose_to_overwrite() {
        let seen = FileStamp {
            mtime: UnixMillis(7),
            hash: ContentHash([3; 32]),
        };
        let cases = [
            (
                "read, not overwriting",
                Some(seen),
                Overwrite::No,
                Some(WriteCheck::Unchanged(seen)),
            ),
            (
                "read, overwriting",
                Some(seen),
                Overwrite::Yes,
                Some(WriteCheck::Overwrite),
            ),
            (
                "overwriting without a stamp",
                None,
                Overwrite::Yes,
                Some(WriteCheck::Overwrite),
            ),
            // Never written blind.
            ("nothing read", None, Overwrite::No, None),
        ];
        for (why, seen, overwrite, want) in cases {
            assert_eq!(write_check(seen, overwrite), want, "{why}");
        }
    }
}
