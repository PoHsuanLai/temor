//! The forwards of one host.

use temor_ssh::{Forward, Port};

/// Why a forward could not be opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ForwardFault {
    /// Something on this computer already listens on the local port; try another.
    LocalPortTaken,
    /// The shared connection is not running.
    NoConnection,
    /// The host would not forward the port.
    Refused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Phase {
    Opening,
    Open,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct Entry {
    pub remote: Port,
    pub local: Port,
    pub phase: Phase,
}

/// The forwards open or being opened on one host's connection, in the order they were asked
/// for. A host port has at most one.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Forwards {
    pub(super) entries: Vec<Entry>,
}

impl Forwards {
    /// The local port the host's `remote` port is forwarded to, once it is open.
    pub fn local_of(&self, remote: Port) -> Option<Port> {
        self.entries
            .iter()
            .find(|e| e.remote == remote && e.phase == Phase::Open)
            .map(|e| e.local)
    }

    /// The host ports forwarded or being forwarded.
    pub fn remote_ports(&self) -> Vec<Port> {
        self.entries.iter().map(|e| e.remote).collect()
    }
}

/// What moves the forwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ForwardIn {
    /// The person asked for the host's `remote` port. `local` is a port this computer has free
    /// (the app probes; the same number as `remote` when it is free).
    Requested { remote: Port, local: Port },
    /// The forward for `remote` is up.
    Opened { remote: Port },
    /// The forward for `remote` could not be opened.
    Failed { remote: Port, fault: ForwardFault },
    /// The person closed the forward for `remote`.
    Cancelled { remote: Port },
    /// The shared connection ended, and its forwards with it.
    ConnectionLost,
}

/// What the forwards ask the app to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForwardOut {
    /// Run `ssh -O forward` for this, then send `Opened` or `Failed`.
    Open(Forward),
    /// Run `ssh -O cancel` for this. Its result does not matter.
    Close(Forward),
    /// `remote` can be used now at `localhost:local`: open it.
    Ready { remote: Port, local: Port },
    /// Tell the person the forward for `remote` failed.
    Report { remote: Port, fault: ForwardFault },
    /// The connection ended; these host ports were forwarded. Offer to forward them again when
    /// it is back.
    Lost(Vec<Port>),
}
