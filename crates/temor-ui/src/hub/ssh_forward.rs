//! Port forwards on a host's shared connection (PLAN §9b "Port forwarding"). The decisions are
//! temor-core's `forward_step`; this runs the `ssh -O forward` it names and tells the person.

use temor_client::model::host::forward::{ForwardFault, ForwardIn, ForwardOut, forward_step};
use temor_client::model::host::master::Master;
use temor_client::model::vocab::ids::PaneId;
use temor_client::ssh::{HostAlias, Port};

use super::model::Hub;
use crate::platform::open_link;
use temor_client::ssh::work::free_local_port;

/// What the person is told when a forward cannot be opened.
pub(super) fn fault_words(host: &HostAlias, remote: Port, fault: ForwardFault) -> String {
    let (host, port) = (host.as_str(), remote.get());
    match fault {
        ForwardFault::LocalPortTaken => {
            format!("Port {port} on {host} can't be forwarded: this computer is already using it")
        }
        ForwardFault::NoConnection => {
            format!("Port {port} on {host} can't be forwarded: the connection isn't running")
        }
        _ => format!("{host} wouldn't forward port {port}"),
    }
}

/// What the person is told when the forward lands on another port than the host's own: that
/// port is in use on this computer, and where the host's port can be reached instead.
pub(super) fn swap_words(host: &HostAlias, remote: Port, local: Port) -> Option<String> {
    match local.get() == remote.get() {
        true => None,
        false => Some(format!(
            "Port {} is in use on this computer, so {} port {} is at localhost:{}",
            remote.get(),
            host.as_str(),
            remote.get(),
            local.get()
        )),
    }
}

impl Hub {
    pub(super) fn feed_forwards(&mut self, alias: &HostAlias, input: ForwardIn) {
        let line = self.ssh.lines.entry(alias.clone()).or_default();
        let (forwards, outs) = forward_step(std::mem::take(&mut line.forwards), input);
        line.forwards = forwards;
        for out in outs {
            match out {
                ForwardOut::Open(forward) => self.ssh.work.open_forward(alias.clone(), forward),
                ForwardOut::Close(forward) => self.ssh.work.close_forward(alias.clone(), forward),
                // The row lists the forward with its Open button; nothing opens by itself.
                ForwardOut::Ready { .. } => {}
                ForwardOut::Report { remote, fault } => {
                    self.notices.push(fault_words(alias, remote, fault));
                }
                ForwardOut::Lost(ports) => {
                    let ports: Vec<String> = ports.iter().map(|p| p.get().to_string()).collect();
                    self.notices.push(format!(
                        "The connection to {} ended, and with it the forwarded ports ({})",
                        alias.as_str(),
                        ports.join(", ")
                    ));
                }
            }
        }
    }

    fn connected_host(&mut self, pane: PaneId) -> Option<HostAlias> {
        let alias = self.alias_of(pane)?;
        let up = self.ssh.lines.get(&alias).map(|line| &line.master) == Some(&Master::Up);
        match up {
            true => Some(alias),
            false => {
                self.notices
                    .push(format!("Wait until {} is connected", alias.as_str()));
                None
            }
        }
    }

    /// Forwards the host's port `text` over the pane's connection to a port on this computer.
    pub fn forward_port(&mut self, pane: PaneId, text: &str) {
        let Some(remote) = Port::parse(text.trim()) else {
            self.notices
                .push("A port is a number from 1 to 65535".to_owned());
            return;
        };
        let Some(alias) = self.connected_host(pane) else {
            return;
        };
        let Some(local) = free_local_port(remote) else {
            self.notices
                .push("This computer has no free port to forward to".to_owned());
            return;
        };
        if let Some(said) = swap_words(&alias, remote, local) {
            self.notices.push(said);
        }
        self.feed_forwards(&alias, ForwardIn::Requested { remote, local });
    }

    /// Closes the forward of the host's port `remote`.
    pub fn cancel_forward(&mut self, pane: PaneId, remote: Port) {
        if let Some(alias) = self.alias_of(pane) {
            self.feed_forwards(&alias, ForwardIn::Cancelled { remote });
        }
    }

    /// Opens the forwarded port in the browser, on the person's click.
    pub fn open_forwarded(&mut self, pane: PaneId, remote: Port) {
        let local = self
            .ssh
            .panes
            .get(&pane)
            .and_then(|host| self.ssh.lines.get(&host.alias))
            .and_then(|line| line.forwards.local_of(remote));
        if let Some(local) = local {
            self.open_url(&format!("http://localhost:{}", local.get()));
        }
    }

    /// Opens the host's port straight over the tailnet (no forward), on the person's click.
    pub fn open_direct(&mut self, pane: PaneId, remote: Port) {
        let direct = self
            .ssh
            .panes
            .get(&pane)
            .and_then(|host| self.ssh.lines.get(&host.alias))
            .and_then(|line| line.direct.clone());
        if let Some(direct) = direct {
            self.open_url(&format!("http://{}", direct.authority(remote)));
        }
    }

    /// Hands a web address to the desktop, if it is one the window opens.
    pub(super) fn open_url(&mut self, url: &str) {
        if let Err(words) = open_link(&*self.ssh.opener, url) {
            self.notices.push(words.to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_forward_on_another_port_says_which() {
        let host = HostAlias::new("prod").expect("a name");
        let port = |n: &str| Port::parse(n).expect("a port");
        assert_eq!(swap_words(&host, port("3000"), port("3000")), None);
        let said = swap_words(&host, port("3000"), port("41234")).unwrap_or_default();
        assert!(
            said.contains("localhost:41234") && said.contains("3000"),
            "{said}"
        );
    }
}
