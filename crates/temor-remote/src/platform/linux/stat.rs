//! The kernel's `stat` as the crate's [`RawStat`].

use rustix::fs::{FileType, Stat};
use temor_wire::message::EntryKind;
use temor_wire::units::UnixMillis;

use crate::platform::RawStat;

// The stat field types differ between targets (i64 or i32, u64 or u32): the conversions are
// needed elsewhere even where they are the same type here.
#[allow(clippy::useless_conversion)]
pub(super) fn raw(stat: &Stat) -> RawStat {
    let kind = match FileType::from_raw_mode(stat.st_mode) {
        FileType::RegularFile => EntryKind::File,
        FileType::Directory => EntryKind::Directory,
        FileType::Symlink => EntryKind::Symlink,
        _ => EntryKind::Other,
    };
    let seconds = i64::try_from(stat.st_mtime).unwrap_or(0);
    let nanos = i64::try_from(stat.st_mtime_nsec).unwrap_or(0);
    RawStat {
        kind,
        len: u64::try_from(stat.st_size).unwrap_or(0),
        mtime: UnixMillis(
            seconds
                .saturating_mul(1000)
                .saturating_add(nanos / 1_000_000),
        ),
        dev: u64::try_from(stat.st_dev).unwrap_or(0),
        ino: u64::try_from(stat.st_ino).unwrap_or(0),
    }
}
