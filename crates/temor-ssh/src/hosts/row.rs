//! One row of the host list.

use crate::config::{HostAlias, HostEntry};
use crate::identity::{HostKey, NodeId};
use crate::tailnet::{DirectHost, Presence, TailnetHost};

/// Which group of the list a row belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum HostGroup {
    /// Hosts the person wrote in `~/.ssh/config`.
    SshConfig,
    /// Tailnet machines with no ssh_config alias ("Your computers").
    YourComputers,
}

/// A host the person can open. The person's own ssh_config alias always wins: a machine
/// that an alias already names is carried by that alias's row and never listed again.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum HostRow {
    /// An ssh_config host, with the tailnet machine its `HostName` points at, if any.
    Configured {
        entry: HostEntry,
        machine: Option<TailnetHost>,
    },
    /// A tailnet machine with no alias.
    Computer(TailnetHost),
}

impl HostRow {
    /// The group the row is listed in.
    pub fn group(&self) -> HostGroup {
        match self {
            Self::Configured { .. } => HostGroup::SshConfig,
            Self::Computer(_) => HostGroup::YourComputers,
        }
    }

    /// The name shown on the row: the alias, or the machine's tailnet name.
    pub fn shown(&self) -> &str {
        match self {
            Self::Configured { entry, .. } => entry.alias.as_str(),
            Self::Computer(machine) => &machine.name,
        }
    }

    /// The tailnet machine behind the row, when there is one.
    pub fn machine(&self) -> Option<&TailnetHost> {
        match self {
            Self::Configured { machine, .. } => machine.as_ref(),
            Self::Computer(machine) => Some(machine),
        }
    }

    /// The machine's node id, when it is a tailnet machine.
    pub fn node(&self) -> Option<&NodeId> {
        self.machine().map(|m| &m.node)
    }

    /// What pins and consent are remembered by before any connection: the node id. `None` for
    /// an alias that is not a tailnet machine, which is remembered by fingerprint once it
    /// connects.
    pub fn key(&self) -> Option<HostKey> {
        self.node().cloned().map(HostKey::from_node)
    }

    /// The destination to pass to `ssh`: the alias, or for a machine without one its MagicDNS
    /// name (else its tailnet address). `None` when nothing usable remains (an IPv6-only
    /// machine with MagicDNS off).
    pub fn destination(&self) -> Option<HostAlias> {
        match self {
            Self::Configured { entry, .. } => Some(entry.alias.clone()),
            Self::Computer(machine) => {
                let dns = machine.dns.trim_end_matches('.');
                HostAlias::new(dns).ok().or_else(|| {
                    machine
                        .addresses
                        .iter()
                        .find_map(|addr| HostAlias::new(&addr.to_string()).ok())
                })
            }
        }
    }

    /// Whether the tailnet sees the machine; `None` for a host that is not a tailnet machine.
    /// The list shows offline machines dimmed.
    pub fn presence(&self) -> Option<Presence> {
        self.machine().map(|m| m.online)
    }

    /// Where this host's ports can be opened directly, for a tailnet machine that is online.
    pub fn direct_host(&self) -> Option<DirectHost> {
        self.machine().and_then(TailnetHost::direct_host)
    }
}
