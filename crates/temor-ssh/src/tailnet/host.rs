//! The host record the desktop layer fills from the person's tailnet (PLAN §9b, D22). It holds
//! no dependency on the account service: temor-desktop reads the list and maps each machine to
//! one [`TailnetHost`]; the host list in the window only reads these.

use std::net::IpAddr;

/// A Tailscale node's stable id. It survives renames and address changes, so pins and consent
/// are kept by it rather than by name or address.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TailnetNode(String);

impl TailnetNode {
    /// The node id as the account service gave it.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// The id's text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Seconds since the Unix epoch, UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UnixSeconds(pub i64);

/// Whether the computer is connected to the tailnet right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Reach {
    /// Connected now.
    Online,
    /// Not connected; it may have a last-seen time.
    Offline,
}

/// Whether Tailscale's own SSH is on for the computer (it needs no key or identity picker).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TailscaleSsh {
    /// The computer takes Tailscale SSH.
    On,
    /// It does not; the usual keys apply.
    Off,
}

/// Whose computer it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TailnetOwner {
    /// The person's own.
    Mine,
    /// Another person's, shared with this one or on the same network.
    Shared,
    /// Run by a tag, such as a server an organisation set up.
    Tagged,
}

/// One computer on the person's tailnet. This computer itself is never listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailnetHost {
    /// Its stable id.
    pub node: TailnetNode,
    /// The name people call it by.
    pub name: String,
    /// Its MagicDNS name, without the trailing dot: what ssh connects to.
    pub dns: String,
    /// Its Tailscale addresses.
    pub addresses: Vec<IpAddr>,
    /// Whether it is connected now.
    pub reach: Reach,
    /// When it was last connected; absent while online and when Tailscale does not know.
    pub last_seen: Option<UnixSeconds>,
    /// Whether Tailscale SSH is on.
    pub ssh: TailscaleSsh,
    /// Whose it is.
    pub owner: TailnetOwner,
}

impl TailnetHost {
    /// Whether an ssh_config `HostName` reaches this computer: its MagicDNS name (any case, with
    /// or without the final dot) or one of its Tailscale addresses. Such an alias is shown once,
    /// under the person's own name for it.
    pub fn is_reached_by(&self, host_name: &str) -> bool {
        let wanted = host_name.trim_end_matches('.');
        if wanted.eq_ignore_ascii_case(&self.dns) {
            return true;
        }
        wanted
            .parse::<IpAddr>()
            .is_ok_and(|address| self.addresses.contains(&address))
    }
}
