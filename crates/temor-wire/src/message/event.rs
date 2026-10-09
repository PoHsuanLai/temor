//! Events: what the helper reports without being asked.

use super::files::{RemotePath, WatchChange};
use super::ports::ListeningPort;
use super::session::ExitStatus;
use crate::ids::{SessionId, WatchId};
use serde::{Deserialize, Serialize};

/// Something the helper reports unasked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Event {
    /// Output for an attached session. The payload is the bytes.
    Output {
        /// The session.
        session: SessionId,
    },
    /// A session's program ended.
    Exited {
        /// The session.
        session: SessionId,
        /// How it ended.
        status: ExitStatus,
    },
    /// The listening ports changed. Sent whole, so the latest report replaces the one before.
    Ports(Vec<ListeningPort>),
    /// A watched path changed.
    FileChanged {
        /// The watch.
        watch: WatchId,
        /// The path that changed.
        path: RemotePath,
        /// How.
        kind: WatchChange,
    },
}
