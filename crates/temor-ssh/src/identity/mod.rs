//! A host's identity (PLAN §9b gate, D18, D22): its SSH host-key fingerprint, the Tailscale
//! node id when it has one, and the name the person sees.

mod fingerprint;
mod host_id;
mod key;
mod node;
#[cfg(test)]
mod tests;

pub use fingerprint::{Fingerprint, ShownName, fingerprint_from_keygen};
pub use host_id::HostId;
pub use key::HostKey;
pub use node::NodeId;
