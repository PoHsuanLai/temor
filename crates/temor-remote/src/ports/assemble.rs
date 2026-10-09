//! Joining listening sockets to the processes that own them.

use std::collections::BTreeMap;

use temor_wire::message::ListeningPort;
use temor_wire::units::{Pid, Port};

use super::parse::ListenSocket;

/// One process of a session's tree, with the socket inodes it holds open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Owner {
    /// The process.
    pub pid: u32,
    /// Its name, as the host shows it.
    pub name: String,
    /// The inodes of the sockets among its open files.
    pub sockets: Vec<u64>,
}

/// The listening ports that a process of the tree owns, once each (a program listening on both
/// the IPv4 and IPv6 sides reports one port), in port order. When several processes share a
/// socket the lowest pid is named, so the answer does not depend on scan order.
pub fn assemble(listening: &[ListenSocket], owners: &[Owner]) -> Vec<ListeningPort> {
    let mut by_port: BTreeMap<u16, ListeningPort> = BTreeMap::new();
    for socket in listening {
        let owner = owners
            .iter()
            .filter(|owner| owner.sockets.contains(&socket.inode))
            .min_by_key(|owner| owner.pid);
        let Some(owner) = owner else { continue };
        by_port.entry(socket.port).or_insert_with(|| ListeningPort {
            port: Port(socket.port),
            pid: Pid(owner.pid),
            process: owner.name.clone(),
        });
    }
    by_port.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner(pid: u32, name: &str, sockets: &[u64]) -> Owner {
        Owner {
            pid,
            name: name.to_string(),
            sockets: sockets.to_vec(),
        }
    }

    #[test]
    fn ports_outside_the_tree_are_left_out_and_dual_stack_counts_once() {
        let listening = [
            ListenSocket {
                port: 8080,
                inode: 1,
            },
            ListenSocket {
                port: 8080,
                inode: 2,
            },
            ListenSocket { port: 22, inode: 3 },
            ListenSocket {
                port: 3000,
                inode: 4,
            },
        ];
        let owners = [
            owner(40, "node", &[4]),
            owner(30, "python", &[2, 9]),
            owner(31, "python-child", &[2]),
        ];
        let got = assemble(&listening, &owners);
        let summary: Vec<(u16, u32, &str)> = got
            .iter()
            .map(|p| (p.port.0, p.pid.0, p.process.as_str()))
            .collect();
        assert_eq!(summary, vec![(3000, 40, "node"), (8080, 30, "python")]);
    }
}
