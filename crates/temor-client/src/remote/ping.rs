//! Noticing a connection that stalled (PLAN §9b): a laptop that slept or a Wi-Fi that dropped
//! leaves the `ssh` open and silent. A thread asks the helper whether it is there every
//! [`PingPolicy`] interval; the question runs on a second thread the first can abandon, because a
//! write to a stalled link can block with no end. No answer in time is reported once, and both
//! threads end. They also end when the connection is dropped.

use std::io;
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::sync::{Arc, Weak};
use std::time::Duration;

use temor_remote::Error;
use temor_wire::link::{LinkEvent, Millis, Monotonic, PingPolicy};

use crate::remote::calls::ping;
use crate::remote::conn::Conn;

/// How long past the wait for an answer the watcher itself waits before it calls a ping that
/// has not even returned unanswered.
const GRACE: Duration = Duration::from_secs(2);

/// What one ping came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pulse {
    Answered,
    /// No answer in time: the link is open but stalled.
    Silent,
    /// The link failed some other way; its closing is reported on its own.
    Broken,
}

/// Sorts the outcome of a ping.
pub fn pulse_of(outcome: &Result<(), Error>) -> Pulse {
    match outcome {
        Ok(()) => Pulse::Answered,
        Err(Error::NoAnswer) => Pulse::Silent,
        Err(_) => Pulse::Broken,
    }
}

impl Pulse {
    /// What the reconnect machine hears of it, if anything.
    pub fn link_event(self, at: Monotonic) -> Option<LinkEvent> {
        match self {
            Self::Silent => Some(LinkEvent::PingTimedOut { at }),
            Self::Answered | Self::Broken => None,
        }
    }
}

fn duration(millis: Millis) -> Duration {
    Duration::from_millis(millis.0)
}

/// Pings `conn` on `policy`'s schedule until it is dropped, fails, or goes silent; silence calls
/// `on_stall` once.
pub fn watch(
    conn: &Arc<Conn>,
    policy: PingPolicy,
    on_stall: impl FnOnce() + Send + 'static,
) -> io::Result<()> {
    let (go, asked) = channel::<Arc<Conn>>();
    let (done, answers) = channel::<Pulse>();
    let within = duration(policy.within);
    std::thread::Builder::new()
        .name("temor-remote-ping".to_owned())
        .spawn(move || {
            while let Ok(conn) = asked.recv() {
                let pulse = pulse_of(&ping(conn.client(), within));
                drop(conn);
                if done.send(pulse).is_err() {
                    break;
                }
            }
        })?;
    let (every, patience, weak): (_, _, Weak<Conn>) =
        (duration(policy.every), within + GRACE, Arc::downgrade(conn));
    std::thread::Builder::new()
        .name("temor-remote-watch".to_owned())
        .spawn(move || {
            loop {
                std::thread::sleep(every);
                let Some(conn) = weak.upgrade() else { return };
                if go.send(conn).is_err() {
                    return;
                }
                match answers.recv_timeout(patience) {
                    Ok(Pulse::Answered) => {}
                    Ok(Pulse::Broken) | Err(RecvTimeoutError::Disconnected) => return,
                    Ok(Pulse::Silent) | Err(RecvTimeoutError::Timeout) => {
                        on_stall();
                        return;
                    }
                }
            }
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_silence_tells_the_reconnect_machine_the_link_stalled() {
        let at = Monotonic(40_000);
        let cases = [
            (Ok(()), None),
            (Err(Error::NoAnswer), Some(LinkEvent::PingTimedOut { at })),
            (Err(Error::UnexpectedReply), None),
        ];
        for (outcome, want) in cases {
            assert_eq!(pulse_of(&outcome).link_event(at), want, "{outcome:?}");
        }
    }
}
