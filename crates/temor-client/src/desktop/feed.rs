//! The Tailscale computers as the window reads them (PLAN §9b, D22). The window and the host
//! list never talk to the account service: they ask a [`TailnetFeed`], which answers from the
//! last list it was given. The binary hands the window the live one; everything else (tests, a
//! desktop without Tailscale) gets [`NoTailnet`].

use std::fmt;
use std::sync::{Arc, Mutex};

use temor_desktop::DesktopError;
use temor_ssh::TailnetHost;

/// The person's other computers on their Tailscale network.
///
/// `Send + Sync`: the live feed is filled from the bus task and read from the window thread.
pub trait TailnetFeed: Send + Sync + fmt::Debug {
    /// The computers now, in the account service's order. Empty means the group is absent: no
    /// Tailscale account, a stopped Tailscale, or an account service that refused temor. Never
    /// an error to show the person.
    fn hosts(&self) -> Vec<TailnetHost>;
}

/// No Tailscale: the group is absent. The default.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoTailnet;

impl TailnetFeed for NoTailnet {
    fn hosts(&self) -> Vec<TailnetHost> {
        Vec::new()
    }
}

/// A list kept up to date by the bus task and read by the window.
#[derive(Debug, Clone, Default)]
pub struct LiveTailnet {
    hosts: Arc<Mutex<Vec<TailnetHost>>>,
}

impl LiveTailnet {
    /// Replaces the list.
    pub fn set(&self, hosts: Vec<TailnetHost>) {
        *self.hosts.lock().unwrap_or_else(|p| p.into_inner()) = hosts;
    }
}

impl TailnetFeed for LiveTailnet {
    fn hosts(&self) -> Vec<TailnetHost> {
        self.hosts.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
}

/// What a read of the account service means for the window: its computers, or no group. A refused
/// or failed read is the same as no Tailscale; the person is not told.
pub fn group_of(read: Result<Vec<TailnetHost>, DesktopError>) -> Vec<TailnetHost> {
    read.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use temor_ssh::{NodeId, Presence, TailnetOwner, TailscaleSsh};

    fn host() -> TailnetHost {
        TailnetHost {
            node: NodeId::new("nA").expect("a node id"),
            name: "box".to_owned(),
            dns: "box.tail1.ts.net".to_owned(),
            addresses: Vec::new(),
            online: Presence::Online,
            last_seen: None,
            ssh: TailscaleSsh::On,
            owner: TailnetOwner::Mine,
        }
    }

    #[test]
    fn a_failed_read_leaves_the_group_absent_and_a_good_one_keeps_it() {
        let refused = Err(DesktopError::bus("read the Tailscale computers", "refused"));
        let cases = [
            (Ok(vec![host()]), 1),
            (Ok(Vec::new()), 0),
            (refused, 0),
            (Err(DesktopError::NoBus), 0),
        ];
        for (read, expected) in cases {
            assert_eq!(group_of(read).len(), expected);
        }
    }
}
