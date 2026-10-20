//! A private session bus for the tests: a `dbus-daemon` with a configuration of its own, in a
//! scratch directory, with no service directories and no connection to the person's session.
//! The daemon is temor-testkit's; this adds the desktop's view of it.

#![cfg(all(target_os = "linux", feature = "quire-desktop"))]

use temor_desktop::DesktopEnv;

/// A running private bus; the daemon stops when this is dropped.
pub struct PrivateBus(temor_testkit::PrivateBus);

impl PrivateBus {
    /// Start a bus. Without `dbus-daemon` the test is skipped, unless `TEMOR_REQUIRE_TOOLS=1`,
    /// when it fails.
    pub fn start() -> Option<PrivateBus> {
        temor_testkit::PrivateBus::start("temor-desktop bus").map(PrivateBus)
    }

    /// An environment whose session bus is this one.
    pub fn env(&self) -> DesktopEnv {
        DesktopEnv::at(self.address().to_owned())
    }

    /// The bus's address, for a connection that serves something on it.
    pub fn address(&self) -> &str {
        self.0.address()
    }

    /// A fresh connection to the bus.
    pub async fn connect(&self) -> zbus::Connection {
        zbus::connection::Builder::address(self.address())
            .expect("the bus address")
            .build()
            .await
            .expect("connect to the private bus")
    }
}
