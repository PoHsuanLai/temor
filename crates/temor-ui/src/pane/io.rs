//! The pane's I/O thread (PLAN §4b). It reads the PTY and writes the terminal under the pane's
//! lock; the terminal's replies go straight back to the PTY and its other events go to the
//! window over a channel that never blocks. It also reaps the program: when the program ends,
//! or when the pane is shut down, the terminal hangs up and the process group is killed after
//! the grace period (FOLLOWUPS, temor-pty).

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, MutexGuard};

use temor_pty::{ChildExit, Pty, ReadOutcome, Ready};
use temor_vt::VtEvent;
use tokio::sync::mpsc::UnboundedSender;

use super::event::PaneEvent;
use super::outbox::Outbox;
use super::reap::{GRACE, POLL, ReapStep, reap_step};
use super::term::Term;

/// What the window asks of the I/O thread.
#[derive(Debug)]
pub(super) enum Command {
    /// The grid changed: tell the program, then say so on `done`.
    Resize {
        size: temor_pty::GridSize,
        done: std::sync::mpsc::SyncSender<()>,
    },
    /// The window is gone: hang up and reap.
    Shutdown,
}

/// Everything the thread owns or shares.
pub(super) struct Io {
    pub term: Arc<Mutex<Term>>,
    pub outbox: Arc<Outbox>,
    pub pty: Pty,
    pub commands: Receiver<Command>,
    pub events: UnboundedSender<PaneEvent>,
}

/// Runs until the program is reaped. Called on the pane's own thread.
pub(super) fn run(mut io: Io) {
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        while let Ok(command) = io.commands.try_recv() {
            match command {
                Command::Resize { size, done } => {
                    // A failed resize leaves the program at its old size; nothing better to do.
                    let _ = io.pty.resize(size);
                    let _ = done.send(());
                }
                Command::Shutdown => {
                    let exit = reap(&mut io, true);
                    send(&io, PaneEvent::Exited(exit));
                    return;
                }
            }
        }
        let interest = if io.outbox.is_waiting() {
            temor_pty::Interest::ReadWrite
        } else {
            temor_pty::Interest::Read
        };
        let ready = match io.outbox.port().wait(interest, Some(POLL)) {
            Ok(ready) => ready,
            Err(_) => {
                let exit = reap(&mut io, true);
                send(&io, PaneEvent::Exited(exit));
                return;
            }
        };
        if ready.write == Ready::Now {
            io.outbox.flush();
        }
        if ready.read != Ready::Now {
            continue;
        }
        match io.outbox.port().read(&mut buf) {
            Ok(ReadOutcome::Data(n)) => deliver(&io, &buf[..n]),
            Ok(ReadOutcome::WouldBlock) => {}
            Ok(ReadOutcome::Closed) | Err(_) => {
                // Every holder of the terminal has closed it: the program is done.
                let exit = reap(&mut io, false);
                send(&io, PaneEvent::Exited(exit));
                return;
            }
        }
    }
}

/// Feeds program output to the terminal, sends its replies to the PTY and its events on.
fn deliver(io: &Io, bytes: &[u8]) {
    let output = lock_term(&io.term).write(bytes);
    for reply in output.replies {
        io.outbox.push(&reply);
    }
    for event in output.events.into_iter().filter_map(map_event) {
        send(io, event);
    }
    send(io, PaneEvent::Dirty);
}

/// The window's view of a terminal event; events the window does not use yet are dropped.
pub(super) fn map_event(event: VtEvent) -> Option<PaneEvent> {
    Some(match event {
        VtEvent::Title(title) => PaneEvent::Title(title),
        VtEvent::Bell => PaneEvent::Bell,
        VtEvent::Cwd(url) => PaneEvent::Cwd(url),
        VtEvent::Clipboard { write } => PaneEvent::Clipboard(write),
        VtEvent::Notification(note) => PaneEvent::Notification(note),
        VtEvent::Prompt(mark) => PaneEvent::Prompt(mark),
        // Progress, program status, resets, render holds, colour-scheme queries and unknown
        // sequences have no UI yet (PLAN §4b, §9); they are dropped here, in one place.
        _ => return None,
    })
}

/// Ends the program: optionally hangs up its process group, polls for the exit for the grace
/// period, then kills it.
fn reap(io: &mut Io, hang_up: bool) -> Option<ChildExit> {
    if hang_up {
        // Already gone is fine: the poll below sees the exit either way.
        let _ = io.pty.hang_up();
    }
    let mut waited = std::time::Duration::ZERO;
    loop {
        if let Ok(Some(exit)) = io.pty.try_exit() {
            return Some(exit);
        }
        match reap_step(waited, GRACE) {
            ReapStep::Poll => {
                std::thread::sleep(POLL);
                waited += POLL;
            }
            ReapStep::Kill => {
                // `kill` reaps the child itself; `None` when the status is no longer known.
                return io.pty.kill().ok();
            }
        }
    }
}

fn send(io: &Io, event: PaneEvent) {
    // A closed channel means the window is gone; the event has nowhere to go.
    let _ = io.events.send(event);
}

/// Locks a pane's terminal. A poisoned lock still holds the terminal, so the pane keeps going.
pub(super) fn lock_term(term: &Mutex<Term>) -> MutexGuard<'_, Term> {
    term.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}
