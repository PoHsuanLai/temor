//! What the helper tells the client unasked.

use temor_wire::ids::{SessionId, WatchId};
use temor_wire::message::{Event, ExitStatus, ListeningPort, RemotePath, WatchChange};

/// Something the helper reported, or the end of the link.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ClientEvent {
    /// Output for an attached session: feed it to that session's terminal.
    Output {
        /// The session.
        session: SessionId,
        /// The bytes.
        bytes: Vec<u8>,
    },
    /// A session's program ended.
    Exited {
        /// The session.
        session: SessionId,
        /// How it ended.
        status: ExitStatus,
    },
    /// The host's listening ports changed; this is the whole list.
    Ports(Vec<ListeningPort>),
    /// A watched path changed.
    FileChanged {
        /// The watch.
        watch: WatchId,
        /// The path.
        path: RemotePath,
        /// How.
        kind: WatchChange,
    },
    /// An event this build could not read in full (it comes from a newer helper).
    Unreadable {
        /// Its kind.
        kind: String,
    },
    /// The link ended. Nothing follows; the window runs its reconnect machine.
    Closed,
}

impl ClientEvent {
    /// The client's view of a wire event; `None` for kinds this build does not know.
    pub(super) fn from_wire(event: Event, payload: Option<Vec<u8>>) -> Option<Self> {
        Some(match event {
            Event::Output { session } => Self::Output {
                session,
                bytes: payload.unwrap_or_default(),
            },
            Event::Exited { session, status } => Self::Exited { session, status },
            Event::Ports(ports) => Self::Ports(ports),
            Event::FileChanged { watch, path, kind } => Self::FileChanged { watch, path, kind },
            _ => return None,
        })
    }
}
