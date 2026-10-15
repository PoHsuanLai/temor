//! A private session bus for the tests: a `dbus-daemon` with a configuration of its own, in a
//! scratch directory, with no service directories and no connection to the person's session.

#![cfg(all(target_os = "linux", feature = "quire-desktop"))]

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use temor_desktop::DesktopEnv;

const CONFIG: &str = r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:path=SOCKET</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"#;

/// A running private bus; the daemon stops when this is dropped.
pub struct PrivateBus {
    daemon: Child,
    address: String,
    _scratch: tempfile::TempDir,
}

impl PrivateBus {
    /// Start a bus. Without `dbus-daemon` the test is skipped, unless `TEMOR_REQUIRE_TOOLS` is
    /// set, when it fails.
    pub fn start() -> Option<PrivateBus> {
        let scratch = tempfile::tempdir().expect("scratch directory");
        let socket = scratch.path().join("bus");
        let config = scratch.path().join("bus.conf");
        std::fs::write(&config, CONFIG.replace("SOCKET", &socket.to_string_lossy()))
            .expect("write the bus configuration");
        let spawned = Command::new("dbus-daemon")
            .arg("--nofork")
            .arg("--print-address")
            .arg(format!("--config-file={}", config.display()))
            .stdout(Stdio::piped())
            .spawn();
        let mut daemon = match spawned {
            Ok(daemon) => daemon,
            Err(error) => {
                assert!(
                    std::env::var_os("TEMOR_REQUIRE_TOOLS").is_none(),
                    "dbus-daemon is required but cannot be started: {error}"
                );
                eprintln!("SKIPPED: dbus-daemon cannot be started ({error})");
                return None;
            }
        };
        let stdout = daemon.stdout.take().expect("the daemon's output");
        let mut address = String::new();
        BufReader::new(stdout)
            .read_line(&mut address)
            .expect("the bus address");
        Some(PrivateBus {
            daemon,
            address: address.trim().to_owned(),
            _scratch: scratch,
        })
    }

    /// An environment whose session bus is this one.
    pub fn env(&self) -> DesktopEnv {
        DesktopEnv::at(self.address.clone())
    }

    /// The bus's address, for a connection that serves something on it.
    pub fn address(&self) -> &str {
        &self.address
    }

    /// A fresh connection to the bus.
    pub async fn connect(&self) -> zbus::Connection {
        zbus::connection::Builder::address(self.address.as_str())
            .expect("the bus address")
            .build()
            .await
            .expect("connect to the private bus")
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}
