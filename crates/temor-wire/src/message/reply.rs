//! Replies: the successful answers to requests.

use super::facts::FolderFacts;
use super::files::{DirEntry, FileMeta, FileStamp};
use super::ports::ListeningPort;
use super::relay::RelayReply;
use super::session::SessionInfo;
use super::trial::TrialVerdict;
use crate::ids::{RemoteSessionId, WatchId};
use crate::units::{ByteCount, Size};
use serde::{Deserialize, Serialize};

/// A successful answer. Each variant says what it carries, and which payload (if any) comes with
/// it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Reply {
    /// The request was done and has nothing to return.
    Ack,
    /// The helper is there: the answer to [`Request::Ping`](super::Request::Ping).
    Pong,
    /// A session was opened. Its id is the helper's to choose.
    Opened {
        /// The new session.
        session: RemoteSessionId,
    },
    /// A session was attached. The payload is its snapshot: the screen state that reproduces it
    /// exactly, then the live output follows as events.
    Attached {
        /// The session.
        session: RemoteSessionId,
        /// Its size now.
        size: Size,
    },
    /// The sessions the host holds for this person.
    Sessions(Vec<SessionInfo>),
    /// The listening ports the helper can see.
    Ports(Vec<ListeningPort>),
    /// A folder's entries.
    Listing(Vec<DirEntry>),
    /// A path's metadata.
    Stat(FileMeta),
    /// A ranged read. The payload is the bytes read; `total` is the file's full length.
    Read {
        /// The file's stamp, to check against on write-back.
        stamp: FileStamp,
        /// The full length of the file.
        total: ByteCount,
    },
    /// A write landed; this is the file's new stamp.
    Written(FileStamp),
    /// A watch was set up.
    Watching {
        /// The new watch.
        watch: WatchId,
    },
    /// A session's folder, as the chips read it.
    Facts(FolderFacts),
    /// A trial's verdict.
    Verdict(TrialVerdict),
    /// A relayed call's answer.
    Relayed(RelayReply),
}
