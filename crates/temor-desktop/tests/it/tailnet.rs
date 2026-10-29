//! The Tailscale computers, read from a fake `org.quire.Tailnet1` on a private bus (what porter's
//! accountd serves; the wire is `Machines() -> a(sa{sv})` and a payload-free `Changed`).

#![cfg(all(target_os = "linux", feature = "quire-desktop"))]

use crate::bus::PrivateBus;
use std::collections::HashMap;
use std::time::Duration;
use temor_desktop::SystemTailnet;
use temor_ssh::TailnetOwner;
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

/// What accountd answers a caller it does not know as the terminal (porter's `dist/callers.toml`
/// names temor by its systemd unit, so a launcher start is refused).
struct RefusingTailnet;

#[zbus::interface(name = "org.quire.Tailnet1")]
impl RefusingTailnet {
    fn machines(&self) -> zbus::fdo::Result<Vec<Row>> {
        Err(zbus::fdo::Error::AccessDenied(
            "not the terminal".to_owned(),
        ))
    }
}

async fn account_service<I: zbus::object_server::Interface>(
    bus: &PrivateBus,
    interface: I,
) -> zbus::Connection {
    let connection = bus.connect().await;
    connection
        .object_server()
        .at("/org/quire/Tailnet1", interface)
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
    let service = account_service(&bus, FakeTailnet).await;
    let tailnet = SystemTailnet::connect(&bus.env()).await.unwrap();

    let mut changes = tailnet.changes().await.unwrap();
    let hosts = tailnet.hosts().await.unwrap();
    assert_eq!(hosts.len(), 2);
    let (box_, nas) = (&hosts[0], &hosts[1]);
    assert_eq!(box_.node.as_str(), "nA");
    assert_eq!(box_.dns, "box.tail1.ts.net");
    assert_eq!(
        (box_.online, box_.tailscale_ssh, box_.owner),
        (true, true, TailnetOwner::Mine)
    );
    assert_eq!(
        (nas.online, nas.tailscale_ssh, nas.owner, nas.last_seen),
        (false, false, TailnetOwner::Shared, None)
    );
    assert!(box_.is_named("100.64.0.7"));

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

#[tokio::test]
async fn a_refused_caller_gets_an_error_not_a_list() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let _service = account_service(&bus, RefusingTailnet).await;
    let tailnet = SystemTailnet::connect(&bus.env()).await.unwrap();
    // The window turns both into "no Tailscale group" (temor-ui's `group_of`); neither is empty.
    assert!(tailnet.hosts().await.is_err());
    assert!(tailnet.changes().await.is_err());
}
