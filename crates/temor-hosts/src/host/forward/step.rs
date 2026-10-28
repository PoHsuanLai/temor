//! `forward_step`.

use super::model::{Entry, ForwardIn, ForwardOut, Forwards, Phase};
use temor_ssh::{Forward, Port, RemoteHost};

fn forward_of(entry: &Entry) -> Forward {
    Forward {
        bind: None,
        local: entry.local,
        remote_host: RemoteHost::localhost(),
        remote: entry.remote,
    }
}

fn find(forwards: &Forwards, remote: Port) -> Option<usize> {
    forwards.entries.iter().position(|e| e.remote == remote)
}

/// The forwards after `input`. A reply about a port with no forward (a late one after the
/// person closed it) changes nothing.
pub fn forward_step(mut forwards: Forwards, input: ForwardIn) -> (Forwards, Vec<ForwardOut>) {
    match input {
        ForwardIn::Requested { remote, local } => match find(&forwards, remote) {
            Some(i) => {
                let entry = forwards.entries[i];
                let outs = match entry.phase {
                    Phase::Open => vec![ForwardOut::Ready {
                        remote,
                        local: entry.local,
                    }],
                    Phase::Opening => vec![],
                };
                (forwards, outs)
            }
            None => {
                let entry = Entry {
                    remote,
                    local,
                    phase: Phase::Opening,
                };
                forwards.entries.push(entry);
                (forwards, vec![ForwardOut::Open(forward_of(&entry))])
            }
        },
        ForwardIn::Opened { remote } => match find(&forwards, remote) {
            Some(i) if forwards.entries[i].phase == Phase::Opening => {
                forwards.entries[i].phase = Phase::Open;
                let local = forwards.entries[i].local;
                (forwards, vec![ForwardOut::Ready { remote, local }])
            }
            _ => (forwards, vec![]),
        },
        ForwardIn::Failed { remote, fault } => match find(&forwards, remote) {
            Some(i) if forwards.entries[i].phase == Phase::Opening => {
                forwards.entries.remove(i);
                (forwards, vec![ForwardOut::Report { remote, fault }])
            }
            _ => (forwards, vec![]),
        },
        ForwardIn::Cancelled { remote } => match find(&forwards, remote) {
            Some(i) => {
                let entry = forwards.entries.remove(i);
                (forwards, vec![ForwardOut::Close(forward_of(&entry))])
            }
            None => (forwards, vec![]),
        },
        ForwardIn::ConnectionLost => match forwards.entries.is_empty() {
            true => (forwards, vec![]),
            false => {
                let lost = forwards.remote_ports();
                (Forwards::default(), vec![ForwardOut::Lost(lost)])
            }
        },
    }
}
