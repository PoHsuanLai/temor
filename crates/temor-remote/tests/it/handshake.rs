//! The greeting: versions are agreed as `temor-wire` defines, capabilities are the shared ones,
//! and a request outside them is refused before anything is sent.

use temor_remote::{Client, DAEMON_CAPS, Error, hello};
use temor_wire::hello::{Capability, Hello, Incompatibility, ProtocolVersion};

use crate::support::{HELPER, Rig};

#[test]
fn the_agreement_carries_the_helper_and_the_shared_capabilities() {
    let rig = Rig::new("hello");
    let conn = rig.connect();
    let agreement = conn.client.agreement();
    assert_eq!(agreement.helper, HELPER);
    assert_eq!(agreement.caps, DAEMON_CAPS.to_vec());
}

#[test]
fn a_client_without_a_capability_cannot_use_it() {
    let rig = Rig::new("hello-caps");
    let conn = rig.connect_with(hello(HELPER, &[Capability::Files]));
    assert_eq!(conn.client.agreement().caps, vec![Capability::Files]);
    assert!(matches!(
        conn.client.list_sessions(),
        Err(Error::NotOffered(Capability::Sessions))
    ));
    assert!(
        conn.client
            .list_dir(&temor_wire::message::RemotePath(
                rig.files.display().to_string()
            ))
            .is_ok()
    );
}

#[test]
fn ends_that_share_no_version_part_ways_and_both_say_why() {
    let rig = Rig::new("hello-version");
    let future = Hello {
        protocol: ProtocolVersion(9),
        oldest_protocol: ProtocolVersion(7),
        helper: HELPER,
        caps: DAEMON_CAPS.to_vec(),
    };
    let (served_in, client_out) = std::io::pipe().expect("pipe");
    let (client_in, served_out) = std::io::pipe().expect("pipe");
    let daemon = std::sync::Arc::clone(&rig.daemon);
    let serving = std::thread::spawn(move || daemon.serve(served_in, served_out));

    let client = Client::connect(client_in, client_out, future);
    assert!(matches!(
        client,
        Err(Error::Incompatible(
            Incompatibility::NoCommonProtocol { .. }
        ))
    ));
    let served = serving.join().expect("the serving thread did not panic");
    assert!(matches!(
        served,
        Err(Error::Incompatible(
            Incompatibility::NoCommonProtocol { .. }
        ))
    ));
}
