//! What every platform reports about a file.

use temor_wire::message::EntryKind;
use temor_wire::units::UnixMillis;

/// A file's state, read without following links.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawStat {
    /// What it is.
    pub kind: EntryKind,
    /// Its length in bytes.
    pub len: u64,
    /// Its modification time.
    pub mtime: UnixMillis,
    /// The device it is on.
    pub dev: u64,
    /// Its inode number.
    pub ino: u64,
}
