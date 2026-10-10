//! The host list: SSH config hosts and tailnet machines merged, grouped, with offline ones
//! dimmed, read from a scratch `~/.ssh/config`.

use std::sync::Arc;

use ds::base::vocab::Availability;
use temor_ssh::{NodeId, Presence, TailnetHost, TailnetOwner, TailscaleSsh};

use super::support::Rig;
use crate::edge::desktop_feed::TailnetFeed;
use crate::hub::hosts::HostRows;
use crate::view::host_list::{group_title, row_availability, row_detail};

#[derive(Debug)]
struct Fixed(Vec<TailnetHost>);

impl TailnetFeed for Fixed {
    fn hosts(&self) -> Vec<TailnetHost> {
        self.0.clone()
    }
}

fn machine(node: &str, name: &str, dns: &str, online: Presence) -> TailnetHost {
    TailnetHost {
        node: NodeId::new(node).expect("node id"),
        name: name.to_owned(),
        dns: dns.to_owned(),
        addresses: vec![],
        online,
        last_seen: None,
        ssh: TailscaleSsh::Off,
        owner: TailnetOwner::Mine,
    }
}

#[test]
fn the_list_has_each_machine_once_grouped_with_offline_ones_dimmed() {
    let Some(mut rig) = Rig::new("hosts") else {
        return;
    };
    let ssh_dir = rig.dir.parent().expect("root").join("home").join(".ssh");
    std::fs::create_dir_all(&ssh_dir).expect("ssh folder");
    std::fs::write(
        ssh_dir.join("config"),
        "Host prod\n  HostName prod.example.com\nHost box\n  HostName box.tail.ts.net\n",
    )
    .expect("config");
    rig.hub.set_tailnet(Arc::new(Fixed(vec![
        machine("n1", "box", "box.tail.ts.net", Presence::Online),
        machine("n2", "laptop", "laptop.tail.ts.net", Presence::Offline),
        machine("n3", "desk", "desk.tail.ts.net", Presence::Online),
    ])));

    let rows = rig.hub.host_rows();
    // The alias that names a machine carries it; the rest follow, online first.
    let shown: Vec<&str> = rows.iter().map(|row| row.shown()).collect();
    assert_eq!(shown, ["prod", "box", "desk", "laptop"]);
    let groups: Vec<&str> = rows.iter().map(|row| group_title(row.group())).collect();
    assert_eq!(
        groups,
        [
            "Your hosts",
            "Your hosts",
            "Your computers",
            "Your computers"
        ]
    );
    assert_eq!(
        rows[1].presence(),
        Some(Presence::Online),
        "box is the tailnet machine"
    );
    assert_eq!(rows[0].presence(), None);

    let looks: Vec<(Availability, Option<String>)> = rows
        .iter()
        .map(|row| (row_availability(row), row_detail(row)))
        .collect();
    assert_eq!(
        looks,
        [
            (Availability::Enabled, Some("prod.example.com".to_owned())),
            (Availability::Enabled, Some("box.tail.ts.net".to_owned())),
            (Availability::Enabled, None),
            (Availability::Disabled, Some("Offline".to_owned())),
        ]
    );

    // Opening a machine's row makes a tab on its tailnet name, remembered by its node.
    rig.hub.open_host(&rows[2]);
    let pane = rig.front();
    assert_eq!(rig.hub.host_of(pane), Some("desk.tail.ts.net"));
}
