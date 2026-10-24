//! A session's pump: the thread that reads its terminal, feeds the screen and the attached
//! client, and reaps the program when the terminal closes.

use std::sync::Arc;
use std::time::Duration;

use temor_pty::{Interest, PtyIo, ReadOutcome, Ready};

use super::core::Daemon;
use super::input::Backlog;
use super::session::Session;

/// How long a wait for the terminal lasts before the loop looks around again.
const POLL: Duration = Duration::from_millis(200);

/// The most bytes read from the terminal at once, so one frame carries one read.
const CHUNK: usize = 32 * 1024;

/// The most reads in one drain while a program is reaped (a megabyte): a program that ignores
/// the hangup and writes without end cannot keep the reaper from its next look and the kill.
const DRAIN_READS: usize = 32;

/// Runs until the program is reaped, then drops the session from the daemon.
pub(super) fn run(daemon: Arc<Daemon>, session: Arc<Session>, io: PtyIo) {
    let mut buffer = vec![0u8; CHUNK];
    loop {
        if session.close_asked() {
            break;
        }
        let interest = match session.backlog() {
            Backlog::Waiting => Interest::ReadWrite,
            Backlog::Empty => Interest::Read,
        };
        let Ok(ready) = io.wait(interest, Some(POLL)) else {
            break;
        };
        if ready.write == Ready::Now {
            session.flush_input();
        }
        if ready.read != Ready::Now {
            continue;
        }
        match io.read(&mut buffer) {
            Ok(ReadOutcome::Data(read)) => {
                session.output(&buffer[..read]);
                session.flush_input();
            }
            Ok(ReadOutcome::WouldBlock) => {}
            // Every holder of the terminal has closed it: the program is done.
            Ok(ReadOutcome::Closed) | Err(_) => break,
        }
    }
    let exit = session.reap(|| {
        // What the program prints while it leaves is not shown to anyone; it is read so the
        // program can finish writing it.
        for _ in 0..DRAIN_READS {
            if !matches!(io.read(&mut buffer), Ok(ReadOutcome::Data(_))) {
                break;
            }
        }
    });
    session.exited(exit);
    daemon.remove_session(session.id);
}
