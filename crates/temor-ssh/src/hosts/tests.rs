use std::net::IpAddr;

use super::{HostGroup, HostRow, merge_hosts};
use crate::identity::NodeId;
use crate::tailnet::{TailnetHost, TailnetOwner};
use crate::testing::load;

fn machine(node: &str, name: &str, dns: &str, addr: &str, online: bool) -> TailnetHost {
    TailnetHost {
        node: NodeId::new(node).expect("node id"),
        name: name.to_owned(),
        dns: dns.to_owned(),
        addresses: vec![addr.parse::<IpAddr>().expect("address")],
        online,
        last_seen: None,
        tailscale_ssh: true,
        owner: TailnetOwner::Mine,
    }
}

fn fleet() -> Vec<TailnetHost> {
    vec![
        machine("n1", "laptop", "laptop.tail1.ts.net", "100.64.0.1", true),
        machine("n2", "nas", "nas.tail1.ts.net", "100.64.0.7", false),
        machine("n3", "phone", "phone.tail1.ts.net", "100.64.0.3", true),
        machine("n4", "Desk", "desk.tail1.ts.net", "100.64.0.4", false),
    ]
}

fn shape(rows: &[HostRow]) -> Vec<(HostGroup, &str, Option<&str>)> {
    rows.iter()
        .map(|r| (r.group(), r.shown(), r.node().map(NodeId::as_str)))
        .collect()
}

#[test]
fn an_alias_pointing_at_a_machine_shows_once_and_the_alias_wins() {
    let list = load(&[(
        "config",
        "Host work\n  HostName LAPTOP.tail1.ts.net\nHost nas-box\n  HostName 100.64.0.7\n\
         Host plain\n  HostName example.com\nHost work2\n  HostName laptop.tail1.ts.net\n",
    )]);
    let rows = merge_hosts(&list, &fleet());
    assert_eq!(
        shape(&rows),
        vec![
            (HostGroup::SshConfig, "work", Some("n1")),
            (HostGroup::SshConfig, "nas-box", Some("n2")),
            (HostGroup::SshConfig, "plain", None),
            (HostGroup::SshConfig, "work2", Some("n1")),
            // The rest: online first, then by name regardless of case.
            (HostGroup::YourComputers, "phone", Some("n3")),
            (HostGroup::YourComputers, "Desk", Some("n4")),
        ]
    );
}

#[test]
fn a_computer_without_an_alias_connects_by_its_magic_dns_name() {
    let rows = merge_hosts(&load(&[("config", "")]), &fleet());
    let destinations: Vec<_> = rows
        .iter()
        .map(|r| r.destination().map(|d| d.as_str().to_owned()))
        .collect();
    assert_eq!(destinations[0].as_deref(), Some("laptop.tail1.ts.net"));
    let mut v6_only = machine("n9", "v6", "", "fd7a::9", true);
    v6_only.addresses = vec!["fd7a::9".parse().expect("address")];
    assert_eq!(HostRow::Computer(v6_only).destination(), None);
}

#[test]
fn only_an_online_machine_has_a_direct_host() {
    let rows = merge_hosts(
        &load(&[("config", "Host nas\n  HostName 100.64.0.7\n")]),
        &fleet(),
    );
    assert_eq!(rows[0].direct_host(), None, "offline nas");
    let phone = rows.iter().find(|r| r.shown() == "phone").expect("phone");
    assert_eq!(
        phone
            .direct_host()
            .map(|h| h.as_str().to_owned())
            .as_deref(),
        Some("phone.tail1.ts.net")
    );
}

#[test]
fn a_node_id_listed_twice_counts_once() {
    let mut machines = fleet();
    machines.push(machine(
        "n3",
        "phone-again",
        "p2.tail1.ts.net",
        "100.64.0.99",
        true,
    ));
    let rows = merge_hosts(&load(&[("config", "")]), &machines);
    assert_eq!(rows.len(), 4);
}
