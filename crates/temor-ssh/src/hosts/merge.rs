//! Merging ssh_config hosts with tailnet machines.

use std::collections::HashSet;

use super::row::HostRow;
use crate::config::HostList;
use crate::identity::NodeId;
use crate::tailnet::{Presence, TailnetHost};

/// The host list: every ssh_config host in file order, each with the tailnet machine its
/// `HostName` names (by MagicDNS name or tailnet address), then the machines no alias names:
/// online first, then by name. A machine appears once, whatever names it has; if several
/// aliases point at one machine, each alias keeps its row and the machine gets no extra row.
/// A node id seen twice in `tailnet` counts once, the first.
pub fn merge_hosts(list: &HostList, tailnet: &[TailnetHost]) -> Vec<HostRow> {
    let mut seen: HashSet<&NodeId> = HashSet::new();
    let machines: Vec<&TailnetHost> = tailnet.iter().filter(|m| seen.insert(&m.node)).collect();
    let mut named: HashSet<&NodeId> = HashSet::new();
    let mut rows: Vec<HostRow> = list
        .entries
        .iter()
        .map(|entry| {
            let machine = machines
                .iter()
                .find(|m| m.is_named(entry.hostname.as_str()))
                .copied();
            if let Some(machine) = machine {
                named.insert(&machine.node);
            }
            HostRow::Configured {
                entry: entry.clone(),
                machine: machine.cloned(),
            }
        })
        .collect();
    let mut rest: Vec<&TailnetHost> = machines
        .into_iter()
        .filter(|m| !named.contains(&m.node))
        .collect();
    rest.sort_by(|a, b| {
        let rank = |m: &TailnetHost| match m.online {
            Presence::Online => 0,
            _ => 1,
        };
        rank(a)
            .cmp(&rank(b))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    rows.extend(rest.into_iter().cloned().map(HostRow::Computer));
    rows
}
