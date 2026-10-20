//! Port detection against the real `/proc`: a listener started inside a session is reported, to a
//! connection that arrives later too.

use temor_remote::ClientEvent;
use temor_wire::units::Port;

use crate::support::{Rig, tools_present};

/// A listener on an ephemeral loopback port that prints it and then waits.
const LISTENER: &str = "python3 -c \"import socket,time; s=socket.socket(); s.bind(('127.0.0.1',0)); s.listen(); print('PORT=%d;' % s.getsockname()[1], flush=True); time.sleep(60)\"\n";

fn python_present() -> bool {
    let found = temor_pty::find_program("python3", std::ffi::OsStr::new("/usr/bin:/bin")).is_some();
    if !found {
        assert!(
            !std::env::var("TEMOR_REQUIRE_TOOLS").is_ok_and(|v| v == "1"),
            "python3 is required (TEMOR_REQUIRE_TOOLS=1)"
        );
        eprintln!("SKIPPED: needs python3 (set TEMOR_REQUIRE_TOOLS=1 to fail instead)");
    }
    found
}

/// The port the listener printed. The typed command shows `PORT=%d;`, which does not parse.
fn port_in(output: &str) -> Option<u16> {
    output
        .split("PORT=")
        .skip(1)
        .find_map(|rest| rest.split(';').next()?.parse().ok())
}

#[test]
fn a_new_connection_is_told_the_ports_already_known() {
    if !tools_present() || !python_present() {
        return;
    }
    let rig = Rig::new("ports-late");
    let mut first = rig.connect();
    let id = first.client.open_session(rig.bash()).expect("open");
    first.client.attach(id).expect("attach");
    first
        .client
        .send_input(id, LISTENER.as_bytes())
        .expect("input");
    let output = first
        .watch
        .output_has("the port", |out| port_in(out).is_some());
    let port = port_in(&output).expect("a port was printed");
    rig.daemon.tick();

    let mut second = rig.connect();
    second.watch.event("the known ports", |event| match event {
        ClientEvent::Ports(ports) if ports.iter().any(|p| p.port == Port(port)) => Some(()),
        _ => None,
    });
}
