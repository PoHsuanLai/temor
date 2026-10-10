//! The host list as the window offers it (PLAN §9b, D22): the person's `~/.ssh/config` hosts
//! and their tailnet machines in one list, each machine once. The command bar and the host list
//! view read it through [`HostRows`]; the tailnet machines come from a [`TailnetFeed`] the
//! desktop edge provides.

use std::sync::Arc;

use temor_ssh::{HostRow, TailnetHost, merge_hosts};

use super::model::{Hub, HubRef};

/// The rows of the host list, in the order to show them: group them by [`HostRow::group`], dim
/// the offline ones ([`HostRow::presence`]) and connect with [`HostRow::destination`].
pub trait HostRows {
    fn host_rows(&self) -> Vec<HostRow>;
}

/// The person's tailnet machines, as the desktop edge learns them. Read each time the list is
/// shown, so it must be cheap (a cached copy). `Send` and `Sync`: the desktop updates it from its
/// own task.
pub trait TailnetFeed: Send + Sync {
    fn tailnet_hosts(&self) -> Vec<TailnetHost>;
}

/// No tailnet: the list has the SSH hosts alone.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoTailnet;

impl TailnetFeed for NoTailnet {
    fn tailnet_hosts(&self) -> Vec<TailnetHost> {
        Vec::new()
    }
}

/// A tailnet feed as it is handed to the window's root as context.
#[derive(Clone)]
pub struct TailnetHandle(pub Arc<dyn TailnetFeed>);

impl std::fmt::Debug for TailnetHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TailnetHandle")
    }
}

impl HostRows for Hub {
    fn host_rows(&self) -> Vec<HostRow> {
        merge_hosts(&self.ssh.env.host_list(), &self.ssh.tailnet.tailnet_hosts())
    }
}

impl HostRows for HubRef {
    fn host_rows(&self) -> Vec<HostRow> {
        self.0.borrow().host_rows()
    }
}

impl Hub {
    /// Where the tailnet machines come from from now on.
    pub fn set_tailnet(&mut self, feed: Arc<dyn TailnetFeed>) {
        self.ssh.tailnet = feed;
    }
}
