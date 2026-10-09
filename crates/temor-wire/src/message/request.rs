//! Requests: everything temor asks the helper to do.

use super::files::{RemotePath, WriteCheck};
use super::relay::RelayCall;
use super::session::{IdleLimit, OpenSession};
use super::trial::TrialRequest;
use crate::ids::{SessionId, WatchId};
use crate::units::{ByteRange, Size};
use serde::{Deserialize, Serialize};

/// What temor asks for. Requests that carry bytes say so: the bytes are the frame's payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Request {
    /// Open a session. Replies [`Reply::Opened`](super::Reply::Opened).
    OpenSession(OpenSession),
    /// Attach to a session. Replies with its snapshot as the payload.
    AttachSession {
        /// The session.
        session: SessionId,
    },
    /// Detach a session, which keeps running. Replies `Ack`.
    DetachSession {
        /// The session.
        session: SessionId,
    },
    /// End a session: send it a hangup, then reap its program and drop its screen. Replies `Ack`.
    /// Unlike detaching, nothing of the session survives.
    Close {
        /// The session.
        session: SessionId,
    },
    /// Resize a session. Replies `Ack`.
    ResizeSession {
        /// The session.
        session: SessionId,
        /// The new size.
        size: Size,
    },
    /// Input for a session. The payload is the bytes. Replies `Ack`.
    SendInput {
        /// The session.
        session: SessionId,
    },
    /// List the person's sessions. Replies [`Reply::Sessions`](super::Reply::Sessions).
    ListSessions,
    /// Set how long detached sessions are kept. Replies `Ack`.
    SetIdleLimit {
        /// The limit.
        limit: IdleLimit,
    },
    /// List the listening ports. Replies [`Reply::Ports`](super::Reply::Ports).
    ListPorts,
    /// List a folder. Replies [`Reply::Listing`](super::Reply::Listing).
    ListDir {
        /// The folder.
        path: RemotePath,
    },
    /// Stat a path. Replies [`Reply::Stat`](super::Reply::Stat).
    Stat {
        /// The path.
        path: RemotePath,
    },
    /// Read a span of a file. Replies with the bytes as the payload.
    ReadFile {
        /// The file.
        path: RemotePath,
        /// The span.
        range: ByteRange,
    },
    /// Write a file. The payload is the new contents. Replies `Written` or a `Conflict`.
    WriteFile {
        /// The file.
        path: RemotePath,
        /// What must be there first.
        check: WriteCheck,
    },
    /// Watch a path for changes. Replies `Watching`.
    Watch {
        /// The path.
        path: RemotePath,
    },
    /// Stop a watch. Replies `Ack`.
    Unwatch {
        /// The watch.
        watch: WatchId,
    },
    /// Run a proposal's trial. Replies [`Reply::Verdict`](super::Reply::Verdict).
    RunTrial(TrialRequest),
    /// An agent's call, carried out in a remote workspace.
    Relay {
        /// The workspace's folder on the host; relayed paths resolve inside it.
        workspace: RemotePath,
        /// The call.
        call: RelayCall,
    },
}
