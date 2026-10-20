//! The greeting's contents.

use crate::digest::ContentHash;
use serde::{Deserialize, Serialize};

/// A version of this protocol. Each side speaks a contiguous range of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProtocolVersion(pub u16);

/// The versions a side speaks: from `oldest` through `newest`, both inclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProtocolRange {
    /// The oldest version still understood.
    pub oldest: ProtocolVersion,
    /// The newest version spoken.
    pub newest: ProtocolVersion,
}

/// The helper's identity: the hash of its binary. temor uploads one helper per hash, so a
/// mismatch means the host runs a helper that is not the one temor shipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HelperVersion(pub ContentHash);

/// The helper's folder under a host's cache folder (`~/.cache`), and under the runtime folder
/// where there is no runtime directory.
pub const HELPER_CACHE_FOLDER: &str = "temor-remote";

impl HelperVersion {
    /// Where the build with this hash lives under the host's cache folder:
    /// `temor-remote/<hash>/temor-remote`, one folder per build, so an update never overwrites a
    /// build that sessions still run.
    pub fn cache_path(hash: ContentHash) -> String {
        format!("{HELPER_CACHE_FOLDER}/{}/temor-remote", hash.to_hex())
    }
}

/// What a side can do. Agreement is the set both sides offer, in the order of this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Capability {
    /// Sessions that outlive a connection, with idle limits.
    Sessions,
    /// Listening-port reports.
    Ports,
    /// File listing, reading, writing and watching.
    Files,
    /// Proposal trial runs on the host.
    Trial,
    /// Agent relay: fs and terminal calls for a remote workspace.
    Relay,
    /// A session's folder read for the chips (git, version files). Added after the first
    /// protocol versions: an older helper does not offer it, and the window shows no chips for
    /// a host that does not.
    Facts,
    /// A capability this build does not know. Never agreed.
    #[serde(other)]
    Unknown,
}

/// The first message each side sends, before anything else.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    /// The newest protocol version this side speaks.
    pub protocol: ProtocolVersion,
    /// The oldest protocol version this side still speaks. Equal to `protocol` for a side that
    /// speaks one version only.
    pub oldest_protocol: ProtocolVersion,
    /// The helper's identity.
    pub helper: HelperVersion,
    /// What this side can do, in its own order.
    pub caps: Vec<Capability>,
}

impl Hello {
    /// The versions this greeting claims to speak.
    pub fn range(&self) -> ProtocolRange {
        ProtocolRange {
            oldest: self.oldest_protocol,
            newest: self.protocol,
        }
    }
}
