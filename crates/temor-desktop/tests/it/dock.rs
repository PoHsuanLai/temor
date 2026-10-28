//! The dock badge goes out as a `LauncherEntry.Update` broadcast any dock can read.

#![cfg(all(target_os = "linux", feature = "quire-desktop"))]

use crate::bus::PrivateBus;
use futures_util::StreamExt;
use std::collections::HashMap;
use std::time::Duration;
use temor_desktop::{Badge, BadgeCount, SystemDock};
use zbus::zvariant::OwnedValue;
use zbus::{MatchRule, MessageStream};

#[tokio::test]
async fn a_badge_is_broadcast_for_temors_desktop_entry() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let listener = bus.connect().await;
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("com.canonical.Unity.LauncherEntry")
        .unwrap()
        .member("Update")
        .unwrap()
        .build();
    let mut updates = MessageStream::for_match_rule(rule, &listener, None)
        .await
        .unwrap();

    let dock = SystemDock::connect(&bus.env()).await.unwrap();
    let badge = Badge {
        count: Some(BadgeCount(2)),
        progress: None,
        attention: true,
    };
    dock.show(&badge).await.unwrap();

    let message = tokio::time::timeout(Duration::from_secs(5), updates.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let (uri, props): (String, HashMap<String, OwnedValue>) = message.body().deserialize().unwrap();
    assert_eq!(uri, "application://org.quire.Temor.desktop");
    assert_eq!(
        i64::try_from(props["count"].try_clone().unwrap()).unwrap(),
        2
    );
    assert!(bool::try_from(props["urgent"].try_clone().unwrap()).unwrap());
}
