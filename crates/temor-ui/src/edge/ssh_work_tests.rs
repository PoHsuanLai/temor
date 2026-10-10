//! The classifying and port-picking helpers of the SSH jobs.

use temor_core::host::edit::EditFault;
use temor_core::host::forward::ForwardFault;
use temor_ssh::{Port, SshError};

use super::ssh_work::{edit_fault, forward_fault, free_local_port};

#[test]
fn failures_read_as_no_connection_or_a_refusal() {
    let remote = |code| SshError::RemoteFile {
        what: "x",
        code,
        detail: String::new(),
    };
    let spawn = SshError::Spawn {
        program: "ssh".to_owned(),
        source: std::io::Error::other("gone"),
    };
    assert_eq!(edit_fault(&remote(Some(255))), EditFault::NoConnection);
    assert_eq!(edit_fault(&spawn), EditFault::NoConnection);
    assert_eq!(edit_fault(&remote(Some(1))), EditFault::Refused);
    assert_eq!(edit_fault(&remote(None)), EditFault::Refused);
}

#[test]
fn forward_failures_are_told_apart_by_what_ssh_printed() {
    let rows = [
        (
            "bind [127.0.0.1]:8080: Address already in use",
            ForwardFault::LocalPortTaken,
        ),
        (
            "Control socket connect(/x): No such file or directory",
            ForwardFault::NoConnection,
        ),
        (
            "mux_client_forward: forwarding request failed",
            ForwardFault::NoConnection,
        ),
        (
            "channel 2: open failed: administratively prohibited",
            ForwardFault::Refused,
        ),
        ("", ForwardFault::Refused),
    ];
    for (said, want) in rows {
        assert_eq!(forward_fault(said), want, "{said}");
    }
}

#[test]
fn a_taken_port_is_swapped_for_a_free_one() {
    let held = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind");
    let taken = Port::parse(&held.local_addr().expect("addr").port().to_string()).expect("port");
    let got = free_local_port(taken).expect("a free port");
    assert_ne!(got, taken);
}
