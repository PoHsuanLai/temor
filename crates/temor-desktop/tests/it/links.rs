//! A link goes to the desktop portal's `OpenURI`; with no portal on the bus the call fails, so
//! the window can fall back to its own opener.

#![cfg(all(target_os = "linux", feature = "quire-desktop"))]

use crate::bus::PrivateBus;
use std::collections::HashMap;
use std::time::Duration;
use temor_desktop::SystemLinks;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use zbus::interface;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

/// A stand-in portal that reports each link it is asked to open.
struct Portal {
    opened: UnboundedSender<(String, String)>,
}

#[interface(name = "org.freedesktop.portal.OpenURI")]
impl Portal {
    #[zbus(name = "OpenURI")]
    fn open_uri(
        &self,
        parent_window: String,
        uri: String,
        _options: HashMap<String, OwnedValue>,
    ) -> OwnedObjectPath {
        let _ = self.opened.send((parent_window, uri));
        OwnedObjectPath::try_from("/org/freedesktop/portal/desktop/request/1_1/t").unwrap()
    }
}

#[tokio::test]
async fn a_link_is_handed_to_the_portal_with_no_parent_window() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let (opened, mut heard) = unbounded_channel();
    let _portal = zbus::connection::Builder::address(bus.address())
        .unwrap()
        .name("org.freedesktop.portal.Desktop")
        .unwrap()
        .serve_at("/org/freedesktop/portal/desktop", Portal { opened })
        .unwrap()
        .build()
        .await
        .unwrap();

    let links = SystemLinks::connect(&bus.env()).await.unwrap();
    links.open("https://example.org/a?b=c").await.unwrap();

    let call = tokio::time::timeout(Duration::from_secs(5), heard.recv()).await;
    assert_eq!(
        call.unwrap(),
        Some((String::new(), "https://example.org/a?b=c".to_owned()))
    );
}

#[tokio::test]
async fn with_no_portal_on_the_bus_the_link_is_refused() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let links = SystemLinks::connect(&bus.env()).await.unwrap();
    assert!(links.open("https://example.org").await.is_err());
}
