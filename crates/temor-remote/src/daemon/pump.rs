//! A session's pump: the one thread that owns the session. It reads the terminal into the
//! screen, takes the session's commands, feeds both to the [`SessionCore`] and carries out what
//! it answers, and reaps the program when the terminal closes. Being the only owner, it answers
//! an attach with a snapshot and starts sending the output after it with nothing in between.

use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use temor_pty::{ChildExit, Interest, Pty, PtyIo, ReadOutcome, Ready, Wake};
use temor_vt::Terminal;
use temor_wire::ids::RemoteSessionId;
use temor_wire::message::{Event, Message, Reply};

use super::colours::apply_palette;
use super::core::Daemon;
use super::exit::exit_status;
use super::input::Input;
use super::link::Link;
use super::session::{Answer, Command};
use super::session_core::{SessionCore, SessionIn, SessionOut, Subscriber};
use super::sizes::{pty_size, screen_cell, screen_size};
use crate::clock::Clock;
use crate::error::Error;

/// How long a wait for the terminal lasts before the loop looks around again.
const POLL: Duration = Duration::from_millis(200);

/// The most bytes read from the terminal at once, so one frame carries one read.
const CHUNK: usize = 32 * 1024;

/// The most reads in one drain while a program is reaped (a megabyte): a program that ignores
/// the hangup and writes without end cannot keep the reaper from its next look and the kill.
const DRAIN_READS: usize = 32;

/// How long a program may take to leave after its terminal hangs up, and how often that is polled.
const GRACE: Duration = Duration::from_millis(500);
/// How long a killed program is polled (its output drained) before the wait blocks.
const KILLED: Duration = Duration::from_secs(2);
const REAP_POLL: Duration = Duration::from_millis(20);

/// What reaping does next, `waited` after the hang-up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reap {
    /// Give the program its grace period.
    Wait,
    /// The grace is over: kill the whole process group, and wait for it to go.
    KillGroup,
    /// It still has not gone: kill the program and take what status there is.
    Kill,
}

fn reap_step(waited: Duration) -> Reap {
    match waited {
        w if w >= GRACE + KILLED => Reap::Kill,
        w if w >= GRACE => Reap::KillGroup,
        _ => Reap::Wait,
    }
}

/// Everything a session is, owned by its pump thread.
pub(super) struct Pump {
    pub(super) id: RemoteSessionId,
    pub(super) core: SessionCore<Arc<Link>>,
    pub(super) screen: Terminal,
    pub(super) pty: Pty,
    /// The terminal's reading end.
    pub(super) io: PtyIo,
    pub(super) input: Input,
    pub(super) inbox: Receiver<Command>,
    pub(super) wake: Wake,
    pub(super) clock: Arc<dyn Clock>,
}

impl Pump {
    /// Runs until the program is reaped, then drops the session from the daemon.
    pub(super) fn run(mut self, daemon: &Daemon) {
        let mut buffer = vec![0u8; CHUNK];
        loop {
            self.take_commands();
            if self.core.closing() {
                break;
            }
            let interest = match self.input.backlog() {
                true => Interest::ReadWrite,
                false => Interest::Read,
            };
            let Ok(ready) = self.io.wait_or(interest, &self.wake, Some(POLL)) else {
                break;
            };
            if ready.write == Ready::Now {
                self.input.flush();
            }
            if ready.read != Ready::Now {
                continue;
            }
            match self.io.read(&mut buffer) {
                Ok(ReadOutcome::Data(read)) => {
                    self.output(&buffer[..read]);
                    self.input.flush();
                }
                Ok(ReadOutcome::WouldBlock) => {}
                // Every holder of the terminal has closed it: the program is done.
                Ok(ReadOutcome::Closed) | Err(_) => break,
            }
        }
        let exit = self.reap(&mut buffer);
        self.step(SessionIn::Ended(exit_status(exit)), &[]);
        daemon.remove_session(self.id);
    }

    fn take_commands(&mut self) {
        while let Ok(command) = self.inbox.try_recv() {
            self.command(command);
        }
    }

    fn command(&mut self, command: Command) {
        match command {
            Command::Attach(answer) => self.attach(answer),
            Command::Detach(link) => self.step(SessionIn::Left(vec![link]), &[]),
            Command::Input(bytes) => self.input.send(&bytes),
            Command::Reply(bytes) => self.input.reply(&bytes),
            Command::Cwd(cwd) => self.step(SessionIn::Cwd(cwd), &[]),
            Command::Resize { size, cell, answer } => {
                let resized = self
                    .screen
                    .resize(screen_size(size), screen_cell(cell))
                    .map_err(Error::from)
                    .and_then(|()| Ok(self.input.resize(pty_size(size, cell))?));
                if resized.is_ok() {
                    self.step(SessionIn::Resized(size), &[]);
                }
                let _ = answer.give(acked(resized), None);
            }
            Command::Palette { palette, answer } => {
                let _ = answer.give(acked(apply_palette(&mut self.screen, &palette)), None);
            }
            Command::Close => self.step(SessionIn::Close, &[]),
            // Already gone is fine.
            Command::HangUp => drop(self.pty.hang_up()),
            Command::Idle(limit) => self.step(SessionIn::Idle(limit), &[]),
            Command::Tick => self.step(SessionIn::Tick, &[]),
            Command::Info(reply) => drop(reply.send(self.core.info(self.id))),
        }
    }

    /// Answers with the screen's snapshot and from then on sends the output to the link too.
    fn attach(&mut self, answer: Answer) {
        let Some(link) = answer.link().cloned() else {
            return;
        };
        let snapshot = match self.screen.snapshot() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                let _ = answer.give(Err(Error::from(error).to_wire()), None);
                return;
            }
        };
        let reply = Reply::Attached {
            session: self.id,
            size: self.core.size(),
        };
        if answer.give(Ok(reply), Some(snapshot.as_bytes())).is_ok() {
            self.step(SessionIn::Attached(link), &[]);
        }
    }

    /// Feeds program output to the screen and on to every attached client.
    fn output(&mut self, bytes: &[u8]) {
        self.screen.write(bytes);
        self.step(SessionIn::Output, bytes);
    }

    /// Steps the core and carries out its answer; `output` is what the program just wrote.
    fn step(&mut self, input: SessionIn<Arc<Link>>, output: &[u8]) {
        let now = self.clock.now();
        for out in self.core.step(now, input) {
            match out {
                SessionOut::Forward(links) => {
                    let event = Message::Event(Event::Output { session: self.id });
                    // A client that is gone or too slow is dropped; it reattaches for a snapshot.
                    let lost: Vec<_> = links
                        .iter()
                        .filter(|link| link.try_send(&event, Some(output)).is_err())
                        .map(Subscriber::id)
                        .collect();
                    if !lost.is_empty() {
                        self.step(SessionIn::Left(lost), &[]);
                    }
                }
                // Already gone is fine; the loop ends on the core's closing and reaps.
                SessionOut::HangUp => drop(self.pty.hang_up()),
                SessionOut::Exited(links, status) => {
                    let event = Message::Event(Event::Exited {
                        session: self.id,
                        status,
                    });
                    for link in links {
                        // A client that cannot take it has gone; there is no one to tell.
                        let _ = link.try_send(&event, None);
                    }
                }
            }
        }
    }

    /// Waits for the program to leave after its terminal closed: hangs it up, gives it the grace
    /// period, then kills it. `None` when its status is no longer known.
    ///
    /// The terminal is read on every poll, after the kill too: a program exiting on a terminal
    /// nobody reads can stall in its own exit (macOS holds the process there until the output is
    /// taken), so a blocking wait could wait for ever. What it prints is shown to no one. The
    /// commands are taken too, so asking about the session does not wait for the reaping.
    fn reap(&mut self, buffer: &mut [u8]) -> Option<ChildExit> {
        let _ = self.pty.hang_up();
        let mut waited = Duration::ZERO;
        loop {
            if let Ok(Some(exit)) = self.pty.try_exit() {
                return Some(exit);
            }
            match reap_step(waited) {
                Reap::Kill => return self.pty.kill().ok(),
                Reap::KillGroup => {
                    let _ = self.pty.signal_group(temor_pty::SignalKind::Kill);
                }
                Reap::Wait => {}
            }
            for _ in 0..DRAIN_READS {
                if !matches!(self.io.read(buffer), Ok(ReadOutcome::Data(_))) {
                    break;
                }
            }
            self.take_commands();
            std::thread::sleep(REAP_POLL);
            waited += REAP_POLL;
        }
    }
}

fn acked<E: Into<Error>>(
    outcome: Result<(), E>,
) -> Result<Reply, temor_wire::message::RemoteError> {
    outcome
        .map(|()| Reply::Ack)
        .map_err(|error| error.into().to_wire())
}

#[cfg(test)]
mod reap_tests {
    use super::{GRACE, KILLED, Reap, reap_step};
    use std::time::Duration;

    #[test]
    fn a_hung_up_program_has_its_grace_then_its_group_then_itself_killed() {
        let ms = Duration::from_millis;
        assert_eq!(reap_step(Duration::ZERO), Reap::Wait);
        assert_eq!(reap_step(GRACE - ms(1)), Reap::Wait);
        assert_eq!(reap_step(GRACE), Reap::KillGroup);
        assert_eq!(reap_step(GRACE + KILLED - ms(1)), Reap::KillGroup);
        assert_eq!(reap_step(GRACE + KILLED), Reap::Kill);
    }
}
