//! What a host is remembered by: pins and consent are stored against a [`HostKey`].

use super::fingerprint::Fingerprint;
use super::host_id::HostId;
use super::node::NodeId;

/// The remembered identity of a host: its verified fingerprint, its tailnet node id, or both.
/// A tailnet machine known only from the tailnet list has just the node id; a host reached by
/// an ssh_config alias has just the fingerprint, until the tailnet shows it is the same machine.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HostKey {
    fingerprint: Option<Fingerprint>,
    node: Option<NodeId>,
}

impl HostKey {
    /// The key of a connected host.
    pub fn of(id: &HostId) -> Self {
        Self {
            fingerprint: Some(id.fingerprint().clone()),
            node: id.node().cloned(),
        }
    }

    /// The key of a tailnet machine not yet connected to.
    pub fn from_node(node: NodeId) -> Self {
        Self {
            fingerprint: None,
            node: Some(node),
        }
    }

    /// A key from stored parts; `None` when neither part is usable.
    pub fn from_parts(fingerprint: Option<&str>, node: Option<&str>) -> Option<Self> {
        let key = Self {
            fingerprint: fingerprint.and_then(|t| Fingerprint::new(t).ok()),
            node: node.and_then(NodeId::new),
        };
        (key.fingerprint.is_some() || key.node.is_some()).then_some(key)
    }

    /// The fingerprint, when known.
    pub fn fingerprint(&self) -> Option<&Fingerprint> {
        self.fingerprint.as_ref()
    }

    /// The node id, when known.
    pub fn node(&self) -> Option<&NodeId> {
        self.node.as_ref()
    }

    /// Whether the two keys name the same machine: node ids decide when both have one,
    /// otherwise fingerprints decide, and a key with nothing in common with the other is a
    /// different machine.
    pub fn same_machine(&self, other: &HostKey) -> bool {
        match (&self.node, &other.node) {
            (Some(a), Some(b)) => a == b,
            _ => matches!((&self.fingerprint, &other.fingerprint), (Some(a), Some(b)) if a == b),
        }
    }

    /// This key with whatever it lacks taken from `other`, which must be the same machine.
    /// Parts both have keep this key's value.
    pub fn merged(&self, other: &HostKey) -> HostKey {
        HostKey {
            fingerprint: self.fingerprint.clone().or_else(|| other.fingerprint.clone()),
            node: self.node.clone().or_else(|| other.node.clone()),
        }
    }
}
