//! What the jobs and connections of a host's helper tell the window (PLAN §9b), and the tickets
//! the window hands them.

use std::sync::Arc;

use temor_core::vocab::ids::PaneId;
use temor_core::vocab::text::HelperHash;
use temor_pane::Feeder;
use temor_ssh::HostAlias;
use temor_wire::ids::SessionId;
use temor_wire::message::{ListeningPort, Palette};

use super::remote_conn::{Conn, DialFault};
use super::remote_install::Found;
use super::remote_link::SharedTarget;

/// Which connection of a host an event is about; an event of an older one is stale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Generation(pub u64);

impl Generation {
    pub fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }
}

/// Why a pane's session could not be opened or reattached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachFault {
    /// The host no longer has the session (it ended, or its helper was restarted).
    Gone,
    /// The call failed; the link is probably down.
    Failed,
}

/// What a job found, or what a connection told.
#[derive(Debug)]
pub enum RemoteEvent {
    /// What the host has; `None` when it could not be asked.
    Probed {
        host: HostAlias,
        found: Option<Found>,
    },
    /// The helper was stored; the hash of what is there now, `None` when the upload failed.
    Installed {
        host: HostAlias,
        hash: Option<HelperHash>,
    },
    /// A dial finished.
    Dialed {
        host: HostAlias,
        generation: Generation,
        outcome: Result<Arc<Conn>, DialFault>,
    },
    /// A pane's session is open and attached, its screen handed to the pane.
    Attached {
        host: HostAlias,
        generation: Generation,
        pane: PaneId,
        session: SessionId,
    },
    AttachFailed {
        host: HostAlias,
        generation: Generation,
        pane: PaneId,
        fault: AttachFault,
    },
    /// The host's listening ports; the whole list.
    Ports {
        host: HostAlias,
        generation: Generation,
        ports: Vec<ListeningPort>,
    },
    /// A session's program ended.
    Exited {
        host: HostAlias,
        generation: Generation,
        session: SessionId,
    },
    /// The connection ended.
    Closed {
        host: HostAlias,
        generation: Generation,
    },
    /// A ping went unanswered: the connection is open but stalled.
    Stalled {
        host: HostAlias,
        generation: Generation,
    },
    /// The `ssh` that made the shared connection (and asked for passwords in the pane) ended
    /// successfully: the connection is up.
    Connected { pane: PaneId },
}

/// What a pane's session starts from.
#[derive(Debug, Clone)]
pub enum Wanted {
    /// A new session in `cwd` with `env` as its whole environment.
    Fresh {
        cwd: String,
        env: Vec<(String, String)>,
    },
    /// The session the pane had before the link dropped.
    Again(SessionId),
    /// The session the pane had when temor was last quit: adopted if the host still holds it,
    /// else a new one in `cwd` with `env`.
    Resume {
        session: SessionId,
        cwd: String,
        env: Vec<(String, String)>,
    },
}

/// A pane to open or reattach.
#[derive(Debug)]
pub struct Ticket {
    pub pane: PaneId,
    pub target: SharedTarget,
    pub feeder: Feeder,
    pub wanted: Wanted,
    /// The window's colours, for the session to answer colour questions with.
    pub palette: Palette,
}
