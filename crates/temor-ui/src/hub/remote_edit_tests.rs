//! Which way a remote edit goes, and what its write must find.

use temor_core::host::master::Master;
use temor_wire::digest::ContentHash;
use temor_wire::message::{FileStamp, WriteCheck};
use temor_wire::units::UnixMillis;

use super::remote_edit::{EditTier, HelperUp, NoWay, Overwrite, pick_tier, write_check};

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
