//! The Tailscale computers, read from a fake `org.quire.Tailnet1` on a private bus (what porter's
//! accountd serves; the wire is `Machines() -> a(sa{sv})` and a payload-free `Changed`).

#![cfg(all(target_os = "linux", feature = "quire-desktop"))]

use crate::bus::PrivateBus;
use std::collections::HashMap;
use std::time::Duration;
use temor_desktop::SystemTailnet;
use temor_ssh::{Reach, TailnetOwner, TailscaleSsh};
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{OwnedValue, Value};

type Row = (String, HashMap<String, OwnedValue>);

fn owned(value: Value<'_>) -> OwnedValue {
    OwnedValue::try_from(value).unwrap()
}

fn row(node: &str, name: &str, online: bool, ssh: bool, owner: &str) -> Row {
    let fields = HashMap::from([
        ("node".to_owned(), owned(Value::from(node.to_owned()))),
        ("name".to_owned(), owned(Value::from(name.to_owned()))),
        (
            "dns".to_owned(),
            owned(Value::from(format!("{name}.tail1.ts.net"))),
        ),
        (
            "addresses".to_owned(),
            owned(Value::Array(vec!["100.64.0.7".to_owned()].into())),
        ),
        ("os".to_owned(), owned(Value::from("linux".to_owned()))),
        ("online".to_owned(), owned(Value::from(online))),
        ("ssh".to_owned(), owned(Value::from(ssh))),
        ("owner".to_owned(), owned(Value::from(owner.to_owned()))),
    ]);
    (node.to_owned(), fields)
}

struct FakeTailnet;

#[zbus::interface(name = "org.quire.Tailnet1")]
impl FakeTailnet {
    fn machines(&self) -> Vec<Row> {
        vec![
            row("nA", "box", true, true, "mine"),
            row("nB", "nas", false, false, "shared"),
        ]
    }

    #[zbus(signal)]
    async fn changed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}

async fn account_service(bus: &PrivateBus) -> zbus::Connection {
    let connection = bus.connect().await;
    connection
        .object_server()
        .at("/org/quire/Tailnet1", FakeTailnet)
        .await
        .unwrap();
    connection
        .request_name("org.quire.Accounts1")
        .await
        .unwrap();
    connection
}

#[tokio::test]
async fn the_machines_come_back_as_plain_hosts_and_a_change_ticks() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let service = account_service(&bus).await;
    let tailnet = SystemTailnet::connect(&bus.env()).await.unwrap();

    let mut changes = tailnet.changes().await.unwrap();
    let hosts = tailnet.hosts().await.unwrap();
    assert_eq!(hosts.len(), 2);
    let (box_, nas) = (&hosts[0], &hosts[1]);
    assert_eq!(box_.node.as_str(), "nA");
    assert_eq!(box_.dns, "box.tail1.ts.net");
    assert_eq!(
        (box_.reach, box_.ssh, box_.owner),
        (Reach::Online, TailscaleSsh::On, TailnetOwner::Mine)
    );
    assert_eq!(
        (nas.reach, nas.ssh, nas.owner, nas.last_seen),
        (
            Reach::Offline,
            TailscaleSsh::Off,
            TailnetOwner::Shared,
            None
        )
    );
    assert!(box_.is_reached_by("100.64.0.7"));

    let emitter = SignalEmitter::new(&service, "/org/quire/Tailnet1").unwrap();
    FakeTailnet::changed(&emitter).await.unwrap();
    let ticked = tokio::time::timeout(Duration::from_secs(5), changes.next()).await;
    assert_eq!(ticked.unwrap(), Some(()));
}

#[tokio::test]
async fn no_account_service_means_no_computers() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    // The bus is up but nothing owns org.quire.Accounts1 (and nothing is activatable).
    let tailnet = SystemTailnet::connect(&bus.env()).await.unwrap();
    assert!(tailnet.hosts().await.unwrap().is_empty());
}
