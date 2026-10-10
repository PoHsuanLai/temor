//! Joining the desktop on a private session bus (never the person's): the first launch keeps its
//! window's link, and a second launch hands its request over and is told to end. The agent
//! layer, the dock and the Tailscale list are held by temor-desktop's own bus tests.

#![cfg(all(target_os = "linux", feature = "quire-desktop"))]

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use temor_desktop::{DesktopEnv, Request};
use temor_ui::app::desktop::{Joined, join};
use temor_ui::edge::desktop_port::Ask;

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

/// A `dbus-daemon` with a configuration of its own in a scratch directory: no service
/// directories, no connection to the person's session. Stops when dropped.
struct PrivateBus {
    daemon: Child,
    address: String,
    _scratch: tempfile::TempDir,
}

impl PrivateBus {
    /// Without `dbus-daemon` the test is skipped, unless `TEMOR_REQUIRE_TOOLS` is set.
    fn start() -> Option<PrivateBus> {
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
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}

#[test]
fn a_second_launch_is_handed_over_and_the_first_window_hears_its_request() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let env = DesktopEnv::at(bus.address.clone());
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");

    let first = join(runtime.handle(), env.clone(), &Request::NewWindow, None);
    let Joined::First { desktop, trouble } = first else {
        panic!("the first launch must own the name: {first:?}");
    };
    assert_eq!(trouble, None);
    let mut asks = desktop
        .link
        .expect("a link to the window")
        .take_asks()
        .expect("the window's asks");

    let tab = Request::NewTab {
        cwd: Some("/tmp/work".into()),
    };
    for request in [Request::NewWindow, tab] {
        let second = join(runtime.handle(), env.clone(), &request, None);
        assert!(matches!(second, Joined::Handed), "{second:?}");
        let heard = runtime
            .block_on(async { tokio::time::timeout(Duration::from_secs(5), asks.recv()).await });
        match heard {
            Ok(Some(Ask::Launch(got, _))) => assert_eq!(got, request),
            other => panic!("the first window should hear {request:?}, got {other:?}"),
        }
    }
}
