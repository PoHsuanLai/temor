//! Files on the host: paths, stamps for the conflict check, listings and watch events.

use crate::digest::ContentHash;
use crate::units::{ByteCount, ByteOffset, UnixMillis};
use serde::{Deserialize, Serialize};

/// A path on the host, as UTF-8. Paths that are not UTF-8 are refused by the helper rather than
/// mangled.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RemotePath(pub String);

/// A file's state at one moment: when it changed and what it held. Write-back compares both, so a
/// change that keeps the mtime still counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileStamp {
    /// The modification time.
    pub mtime: UnixMillis,
    /// The hash of the contents.
    pub hash: ContentHash,
}

/// What a write must find before it replaces the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WriteCheck {
    /// Write whatever is there.
    Overwrite,
    /// Write only if the file is still exactly this: otherwise the reply is a conflict.
    Unchanged(FileStamp),
    /// Write only if nothing is there yet.
    MustBeNew,
}

/// How a write's payload lands: as the whole file, or as one part of a file sent in several.
///
/// Parts stage in a hidden file beside the target and replace it when the last one arrives, so
/// a reader never sees half a file and the [`WriteCheck`] is applied once, at the end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum WriteMode {
    /// The payload is the whole file.
    #[default]
    Whole,
    /// The payload is the bytes from `offset` on. The first part has offset 0 and starts the
    /// staging file afresh; every later part must start where the staged bytes end, or the
    /// write is refused as a bad request.
    Part {
        /// Where this part starts in the file.
        offset: ByteOffset,
        /// Whether more follow.
        end: PartEnd,
    },
}

/// What a part of a streamed write says about the parts after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PartEnd {
    /// More parts follow. Replies `Ack`.
    More,
    /// The last part: check, replace the file and reply `Written`.
    Last,
    /// Give up: drop what was staged and reply `Ack`. The payload is ignored.
    Abort,
}

/// What kind of entry a path is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntryKind {
    /// A regular file.
    File,
    /// A folder.
    Directory,
    /// A symbolic link (the helper reports what it points at through `stat`).
    Symlink,
    /// Anything else: a device, a socket, a pipe.
    Other,
    /// A kind this build does not know.
    #[serde(other)]
    Unknown,
}

/// What `stat` says about a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileMeta {
    /// The file's stamp.
    pub stamp: FileStamp,
    /// Its length.
    pub len: ByteCount,
    /// What it is.
    pub kind: EntryKind,
}

/// One entry in a folder listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirEntry {
    /// The name, without its folder.
    pub name: String,
    /// What it is.
    pub kind: EntryKind,
    /// Its length.
    pub len: ByteCount,
    /// Its modification time.
    pub mtime: UnixMillis,
}

/// How a watched path changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WatchChange {
    /// Something appeared.
    Created,
    /// Its contents or metadata changed.
    Modified,
    /// It was removed.
    Removed,
    /// It was moved.
    Renamed,
    /// A change this build does not know.
    #[serde(other)]
    Unknown,
}
