//! The host list as the window offers it (PLAN §9b, D22): the person's `~/.ssh/config` hosts
//! and their tailnet machines in one list, each machine once. The command bar reads it through
//! [`HostRows`]; the tailnet machines come from a [`TailnetFeed`] the
//! desktop edge provides.

use std::sync::Arc;

use temor_client::ssh::{HostRow, merge_hosts};

use super::model::{Hub, HubRef};
use temor_client::desktop::feed::TailnetFeed;

/// The rows of the host list, in the order to show them: group them by [`HostRow::group`], dim
/// the offline ones ([`HostRow::presence`]) and connect with [`HostRow::destination`].
pub trait HostRows {
    fn host_rows(&self) -> Vec<HostRow>;
}

impl HostRows for Hub {
    fn host_rows(&self) -> Vec<HostRow> {
        merge_hosts(&self.ssh.env.host_list(), &self.ssh.tailnet.hosts())
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
