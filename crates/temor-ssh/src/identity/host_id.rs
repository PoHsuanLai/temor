//! The identity of a connected host.

use super::fingerprint::{Fingerprint, ShownName};
use super::key::HostKey;
use super::node::NodeId;

/// The identity of a host (PLAN §9b): its SSH host-key fingerprint, the Tailscale node id when
/// it is a tailnet machine, and the name shown for it. Identity checks use
/// [`HostId::same_machine`] and never compare the shown names.
///
/// The fingerprint must come from the host key the live connection verified (for example from
/// `ssh-keygen -l` on the key the handshake presented), never from a config file or from a
/// `known_hosts` line that was merely looked up by name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HostId {
    fingerprint: Fingerprint,
    shown: ShownName,
    node: Option<NodeId>,
}

impl HostId {
    /// Pairs a fingerprint with the name shown for it.
    pub fn new(fingerprint: Fingerprint, shown: ShownName) -> Self {
        Self {
            fingerprint,
            shown,
            node: None,
        }
    }

    /// The same host, known to be this tailnet machine. Pins and consent follow the node id
    /// through a rename or a new host key.
    pub fn with_node(mut self, node: NodeId) -> Self {
        self.node = Some(node);
        self
    }

    /// The host-key fingerprint.
    pub fn fingerprint(&self) -> &Fingerprint {
        &self.fingerprint
    }

    /// The shown name.
    pub fn shown(&self) -> &ShownName {
        &self.shown
    }

    /// The tailnet node id, when the host is a tailnet machine.
    pub fn node(&self) -> Option<&NodeId> {
        self.node.as_ref()
    }

    /// What to remember a host by: its fingerprint and node id, without the shown name.
    pub fn key(&self) -> HostKey {
        HostKey::of(self)
    }

    /// Whether `other` is the same machine. When both carry a node id, the node ids decide
    /// (a machine keeps its identity when its host key changes, and two machines cloned from
    /// one image stay apart). Otherwise equal fingerprints decide. Shown names are ignored.
    pub fn same_machine(&self, other: &HostId) -> bool {
        self.key().same_machine(&other.key())
    }
}
