//! What to offer for a port seen on a host.

use temor_ssh::{DirectHost, Port};

/// One way to reach a port on a host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortOffer {
    /// Open it straight at `host:remote` over the tailnet; nothing is forwarded.
    OpenDirect { host: DirectHost, remote: Port },
    /// Forward it over the shared connection to a port on this computer.
    Forward { remote: Port },
}

/// The ways to reach the host's `remote` port, best first. `direct` is the host's direct name
/// when it is a tailnet machine that is online (`HostRow::direct_host`): then the first offer is
/// to open it there, and forwarding stays available after it.
pub fn offers(direct: Option<DirectHost>, remote: Port) -> Vec<PortOffer> {
    let mut all = Vec::new();
    if let Some(host) = direct {
        all.push(PortOffer::OpenDirect { host, remote });
    }
    all.push(PortOffer::Forward { remote });
    all
}
