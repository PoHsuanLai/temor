//! A private session bus for tests: a `dbus-daemon` with a configuration of its own in a scratch
//! folder, with no service folders and no connection to the person's session.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};

use crate::program::skip;

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
#[derive(Debug)]
pub struct PrivateBus {
    daemon: Child,
    address: String,
    _scratch: tempfile::TempDir,
}

impl PrivateBus {
    /// Starts a bus. Without `dbus-daemon` the test skips, or fails under `TEMOR_REQUIRE_TOOLS=1`
    /// (see [`skip`]); `test` names it in that message.
    pub fn start(test: &str) -> Option<PrivateBus> {
        let scratch = tempfile::tempdir().expect("a scratch folder");
        let socket = scratch.path().join("bus");
        let config = scratch.path().join("bus.conf");
        std::fs::write(&config, CONFIG.replace("SOCKET", &socket.to_string_lossy()))
            .expect("the bus configuration");
        let spawned = Command::new("dbus-daemon")
            .arg("--nofork")
            .arg("--print-address")
            .arg(format!("--config-file={}", config.display()))
            .stdout(Stdio::piped())
            .spawn();
        let mut daemon = match spawned {
            Ok(daemon) => daemon,
            Err(error) => {
                skip(&format!("dbus-daemon ({error})"), test);
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

    /// The bus's address.
    pub fn address(&self) -> &str {
        &self.address
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}
