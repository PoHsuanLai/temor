//! A machine on the person's tailnet (PLAN §9b, D22), as plain data. The desktop edge fills it
//! from the account service; this crate only reads it, to merge the machines into the host list
//! and to decide how to reach a port on one.

use std::net::IpAddr;

use crate::config::Port;
use crate::identity::NodeId;

/// Seconds since the Unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UnixSeconds(pub u64);

/// Whose machine it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TailnetOwner {
    /// The person's own.
    Mine,
    /// Shared with the person by someone else.
    Shared,
    /// Owned by the tailnet through a tag, not by a person.
    Tagged,
}

/// One tailnet machine. This computer is left out by the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailnetHost {
    /// The stable node id; a rename keeps it.
    pub node: NodeId,
    /// The short name the tailnet shows.
    pub name: String,
    /// The MagicDNS name, without a trailing dot. Empty when MagicDNS is off.
    pub dns: String,
    /// The tailnet addresses (`100.x.y.z`, `fd7a:…`).
    pub addresses: Vec<IpAddr>,
    /// The tailnet sees the machine now.
    pub online: bool,
    pub last_seen: Option<UnixSeconds>,
    /// Tailscale's own SSH is on for the machine (no keys needed).
    pub tailscale_ssh: bool,
    pub owner: TailnetOwner,
}

/// A host name or address a tailnet port can be opened at directly.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DirectHost(String);

impl DirectHost {
    /// The `host:port` form for a URL or a socket address; an IPv6 address is bracketed.
    pub fn authority(&self, port: Port) -> String {
        match self.0.contains(':') {
            true => format!("[{}]:{}", self.0, port.get()),
            false => format!("{}:{}", self.0, port.get()),
        }
    }

    /// The bare host or address.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TailnetHost {
    /// Whether `host` (an ssh_config `HostName` value) names this machine: its MagicDNS name,
    /// compared without case or a trailing dot, or one of its addresses.
    pub fn is_named(&self, host: &str) -> bool {
        let host = host.trim_end_matches('.');
        let dns = self.dns.trim_end_matches('.');
        let by_dns = !dns.is_empty() && dns.eq_ignore_ascii_case(host);
        let by_address = host
            .parse::<IpAddr>()
            .is_ok_and(|addr| self.addresses.contains(&addr));
        by_dns || by_address
    }

    /// Where its ports can be opened directly: the MagicDNS name, else the first IPv4 address,
    /// else the first address. `None` when the machine is offline or has no usable name.
    pub fn direct_host(&self) -> Option<DirectHost> {
        if !self.online {
            return None;
        }
        let dns = self.dns.trim_end_matches('.');
        let usable = |text: &str| {
            !text.is_empty()
                && text
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':'))
        };
        let address = self
            .addresses
            .iter()
            .find(|a| a.is_ipv4())
            .or_else(|| self.addresses.first());
        match (usable(dns), address) {
            (true, _) => Some(DirectHost(dns.to_ascii_lowercase())),
            (false, Some(addr)) => Some(DirectHost(addr.to_string())),
            (false, None) => None,
        }
    }
}
