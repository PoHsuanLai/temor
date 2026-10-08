//! SSH hosts (§9b): where a host comes from, what it has been allowed, and which tier it runs.

use crate::vocab::ids::RemoteSessionId;
use crate::vocab::text::{HelperHash, HostAlias, RemotePath, SshTarget};
use ds_core::word::Word;
use ds_style::space::list::SpaceId;
use serde::{Deserialize, Serialize};

/// A host from `~/.ssh/config`, or one typed in the bar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Host {
    /// Its alias.
    pub alias: HostAlias,
    /// What `ssh` connects to.
    pub target: SshTarget,
    /// The Space it is pinned to, if any.
    #[serde(default)]
    pub space_pin: Option<SpaceId>,
    /// What the person has allowed it.
    #[serde(default)]
    pub consent: HostConsent,
    /// Whether the helper runs there.
    #[serde(default)]
    pub tier: RemoteTier,
}

/// What a host may do, each asked once and remembered per Space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct HostConsent {
    /// Reading the clipboard.
    #[serde(default)]
    pub clipboard: Grant,
    /// Sending files.
    #[serde(default)]
    pub transfers: Grant,
    /// Showing notifications.
    #[serde(default)]
    pub notifications: Grant,
    /// Installing the helper.
    #[serde(default)]
    pub helper: Grant,
}

/// One answer about one thing a host may do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Word, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum Grant {
    /// Not asked yet: ask the first time.
    #[default]
    Ask,
    /// Allowed.
    Allowed,
    /// Refused.
    Refused,
}

/// Whether a host runs the helper.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum RemoteTier {
    /// Nothing installed (Tier A).
    #[default]
    Plain,
    /// The helper runs (Tier B).
    Helper {
        /// The build it runs.
        version: HelperHash,
    },
}

/// A durable session the helper keeps on a host.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RemoteSession {
    /// The host.
    pub host: HostAlias,
    /// The session.
    pub id: RemoteSessionId,
    /// Its working directory.
    pub cwd: RemotePath,
}
