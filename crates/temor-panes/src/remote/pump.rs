//! The thread that reads a connection's unasked events: output goes to the panes through the
//! router, everything else to the window (PLAN §9b).

use std::sync::mpsc::Receiver;

use temor_pty::ChildExit;
use temor_remote::ClientEvent;
use temor_ssh::HostAlias;
use temor_wire::message::ExitStatus;
use tokio::sync::mpsc::UnboundedSender;

use crate::remote::event::{Generation, RemoteEvent};
use crate::remote::route::Router;

/// A session's end as the pane reports it.
pub fn exit_of(status: &ExitStatus) -> ChildExit {
    match status {
        ExitStatus::Code(code) => ChildExit::Exited(code.0),
        ExitStatus::Signal { name } => ChildExit::Signaled(signal_number(name)),
    }
}

/// The number of a signal the host named (`SIGKILL`); 0 for one this table does not know.
pub fn signal_number(name: &str) -> i32 {
    match name.trim_start_matches("SIG") {
        "HUP" => 1,
        "INT" => 2,
        "QUIT" => 3,
        "ABRT" => 6,
        "KILL" => 9,
        "SEGV" => 11,
        "PIPE" => 13,
        "TERM" => 15,
        _ => 0,
    }
}

/// Reads `events` until the connection closes, then says so.
pub fn pump(
    host: &HostAlias,
    generation: Generation,
    events: &Receiver<ClientEvent>,
    router: &Router,
    notify: &UnboundedSender<RemoteEvent>,
) {
    while let Ok(event) = events.recv() {
        match event {
            ClientEvent::Output { session, bytes } => router.output(session, bytes),
            ClientEvent::Exited { session, status } => {
                router.ended(session, Some(exit_of(&status)));
                let _ = notify.send(RemoteEvent::Exited {
                    host: host.clone(),
                    generation,
                    session,
                });
            }
            ClientEvent::Ports(ports) => {
                let _ = notify.send(RemoteEvent::Ports {
                    host: host.clone(),
                    generation,
                    ports,
                });
            }
            ClientEvent::Closed => break,
            // Events a newer helper sends that this build cannot read.
            _ => {}
        }
    }
    let _ = notify.send(RemoteEvent::Closed {
        host: host.clone(),
        generation,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use temor_wire::units::ExitCode;

    #[test]
    fn a_session_ends_with_its_code_or_the_signal_the_host_named() {
        let cases = [
            (ExitStatus::Code(ExitCode(0)), ChildExit::Exited(0)),
            (ExitStatus::Code(ExitCode(3)), ChildExit::Exited(3)),
            (
                ExitStatus::Signal {
                    name: "SIGKILL".into(),
                },
                ChildExit::Signaled(9),
            ),
            (
                ExitStatus::Signal {
                    name: "TERM".into(),
                },
                ChildExit::Signaled(15),
            ),
            (
                ExitStatus::Signal {
                    name: "SIGWEIRD".into(),
                },
                ChildExit::Signaled(0),
            ),
        ];
        for (status, want) in cases {
            assert_eq!(exit_of(&status), want, "{status:?}");
        }
    }
}
