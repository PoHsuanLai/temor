//! What to offer for a listening port (PLAN §9b): "Open at <name>:<port>" first when the host is
//! on the person's tailnet, because the port is reachable as it is; "Forward port…" always.

use temor_wire::units::Port;

/// How the host can be reached from this computer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostReach {
    /// A computer on the person's tailnet, by its MagicDNS name.
    Tailnet {
        /// The name to open ports at.
        name: String,
    },
    /// Anything else: ports are reached only through a forward.
    Other,
}

/// One thing the running row can do about a port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortAction {
    /// Open `name:port` directly.
    OpenAt {
        /// The host's name.
        name: String,
        /// The port.
        port: Port,
    },
    /// Forward the port on the shared connection, then open it locally.
    Forward {
        /// The port.
        port: Port,
    },
}

/// The actions for `port` on a host, best first.
pub fn port_actions(reach: &HostReach, port: Port) -> Vec<PortAction> {
    match reach {
        HostReach::Tailnet { name } => vec![
            PortAction::OpenAt {
                name: name.clone(),
                port,
            },
            PortAction::Forward { port },
        ],
        HostReach::Other => vec![PortAction::Forward { port }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tailnet_host_opens_directly_before_it_forwards() {
        let tailnet = HostReach::Tailnet {
            name: "box.tail1234.ts.net".into(),
        };
        assert_eq!(
            port_actions(&tailnet, Port(3000)),
            vec![
                PortAction::OpenAt {
                    name: "box.tail1234.ts.net".into(),
                    port: Port(3000)
                },
                PortAction::Forward { port: Port(3000) },
            ]
        );
        assert_eq!(
            port_actions(&HostReach::Other, Port(22)),
            vec![PortAction::Forward { port: Port(22) }]
        );
    }
}
