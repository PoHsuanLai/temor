//! The host list: SSH config hosts and tailnet machines merged, each machine once, read from a scratch `~/.ssh/config`.

use std::sync::Arc;

use temor_ssh::{NodeId, Presence, TailnetHost, TailnetOwner, TailscaleSsh};

use super::support::Rig;
use crate::hub::hosts::HostRows;
use temor_client::desktop::feed::TailnetFeed;

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
    assert_eq!(
        rows[1].presence(),
        Some(Presence::Online),
        "box is the tailnet machine"
    );
    assert_eq!(rows[0].presence(), None);

    // The bar lists the same hosts (a scratch ssh config, no seam stand-in).
    rig.hub.bar_toggle();
    rig.hub.bar_typed("prod".to_owned());
    let titles: Vec<String> = rig
        .hub
        .bar_data()
        .map(|data| data.rows.into_iter().map(|row| row.title).collect())
        .unwrap_or_default();
    assert!(
        titles.iter().any(|t| t.contains("prod")),
        "the bar lists prod: {titles:?}"
    );
    rig.hub.bar_close();

    // Opening a machine's row makes a tab on its tailnet name, remembered by its node.
    rig.hub.open_host(&rows[2]);
    let pane = rig.front();
    assert_eq!(rig.hub.host_of(pane), Some("desk.tail.ts.net"));
}

#[test]
fn an_offline_machine_says_so_and_connects_only_on_try_anyway() {
    use temor_client::model::host::consent::BootstrapAction;
    use temor_client::ssh::view::Stage;

    let Some(mut rig) = Rig::new("offline") else {
        return;
    };
    rig.hub.set_tailnet(Arc::new(Fixed(vec![machine(
        "n2",
        "laptop",
        "laptop.tail.ts.net",
        Presence::Offline,
    )])));
    let row = rig
        .hub
        .host_rows()
        .into_iter()
        .find(|row| row.shown() == "laptop")
        .expect("the offline machine is listed");

    rig.hub.open_host(&row);
    let pane = rig.front();
    assert_eq!(rig.hub.host_of(pane), Some("laptop.tail.ts.net"));
    assert_eq!(rig.stage(pane), Stage::Offline, "told why, nothing asked");
    assert!(rig.got("session-1").is_none());

    rig.hub.try_offline(pane);
    assert_eq!(rig.stage(pane), Stage::Asking(BootstrapAction::Integrate));
}

#[test]
fn what_a_host_may_do_here_is_remembered_per_host() {
    use temor_client::model::host::consent::Permit;
    use temor_client::model::remote::Grant;
    use temor_client::ssh::HostAlias;

    let Some(mut rig) = Rig::new("permits") else {
        return;
    };
    let prod = HostAlias::new("prod").expect("alias");
    let other = HostAlias::new("other").expect("alias");
    assert_eq!(rig.hub.permit_grant(&prod, Permit::Clipboard), Grant::Ask);
    rig.hub
        .remember_permit(&prod, Permit::Clipboard, Grant::Refused);
    // Host, permit, what the book says.
    for (host, permit, want) in [
        (&prod, Permit::Clipboard, Grant::Refused),
        (&prod, Permit::Transfers, Grant::Ask),
        (&other, Permit::Clipboard, Grant::Ask),
    ] {
        assert_eq!(
            rig.hub.permit_grant(host, permit),
            want,
            "{host:?} {permit:?}"
        );
    }
    rig.hub
        .remember_permit(&prod, Permit::Clipboard, Grant::Ask);
    assert_eq!(rig.hub.permit_grant(&prod, Permit::Clipboard), Grant::Ask);
}
