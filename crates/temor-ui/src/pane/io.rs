//! The pane's I/O thread (PLAN §4b). It reads the PTY and writes the terminal under the pane's
//! lock; the terminal's replies go straight back to the PTY and its other events go to the
//! window. Output is coalesced: the window hears `Dirty` once until it has repainted, so a flood
//! of output costs one event, not one per read. It also reaps the program: when the program ends
//! or the pane is shut down, the terminal hangs up, the process group gets the grace period, and
//! then it is killed.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use temor_pty::{ChildExit, Pty, ReadOutcome, Ready};
use temor_vt::{RenderHold, VtEvent};
use tokio::sync::mpsc::UnboundedSender;

use super::event::PaneEvent;
use super::outbox::Outbox;
use super::reap::{GRACE, POLL, ReapStep, reap_step};
use super::term::Term;

/// What the window asks of the I/O thread.
#[derive(Debug)]
pub(super) enum Command {
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
    /// Set while a `Dirty` is in flight; cleared by the window when it repaints.
    pub dirty: Arc<AtomicBool>,
    /// The last title sent, so a program that sets the same title in a loop costs nothing.
    pub last_title: Option<String>,
}

/// Runs until the program is reaped. Called on the pane's own thread.
pub(super) fn run(mut io: Io) {
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        // The only command is Shutdown (the window dropped the pane).
        if let Ok(Command::Shutdown) = io.commands.try_recv() {
            return finish(&mut io);
        }
        let interest = if io.outbox.is_waiting() {
            temor_pty::Interest::ReadWrite
        } else {
            temor_pty::Interest::Read
        };
        let ready = match io.outbox.port().wait(interest, Some(POLL)) {
            Ok(ready) => ready,
            Err(_) => return finish(&mut io),
        };
        if ready.write == Ready::Now {
            io.outbox.flush();
        }
        if ready.read != Ready::Now {
            continue;
        }
        match io.outbox.port().read(&mut buf) {
            Ok(ReadOutcome::Data(n)) => deliver(&mut io, &buf[..n]),
            Ok(ReadOutcome::WouldBlock) => {}
            // Every holder of the terminal has closed it: the program is done.
            Ok(ReadOutcome::Closed) | Err(_) => return finish(&mut io),
        }
    }
}

/// Reaps the program and tells the window how it ended.
fn finish(io: &mut Io) {
    let exit = reap(io);
    send(io, PaneEvent::Exited(exit));
}

/// Feeds program output to the terminal, sends its replies to the PTY and its events on.
fn deliver(io: &mut Io, bytes: &[u8]) {
    let output = lock_term(&io.term).write(bytes);
    for reply in output.replies {
        let _ = io.outbox.push_reply(&reply);
    }
    for event in output.events {
        if let Some(event) = map_event(event) {
            if let PaneEvent::Title(title) = &event {
                if io.last_title.as_deref() == Some(title.as_str()) {
                    continue;
                }
                io.last_title = Some(title.clone());
            }
            send(io, event);
        }
    }
    signal_dirty(io);
}

/// Tells the window the screen changed, unless it has not repainted the last such signal yet.
pub(super) fn signal_dirty(io: &Io) {
    if !io.dirty.swap(true, Ordering::AcqRel) {
        let _ = io.events.send(PaneEvent::Dirty);
    }
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
        VtEvent::Anchor(id) => PaneEvent::Anchor(id),
        VtEvent::Progress(progress) => PaneEvent::Progress(progress),
        VtEvent::Reset => PaneEvent::Reset,
        VtEvent::RenderHold(RenderHold::Held) => PaneEvent::RenderHold(super::event::Hold::Held),
        VtEvent::RenderHold(RenderHold::Released) => {
            PaneEvent::RenderHold(super::event::Hold::Released)
        }
        // Program status, colour-scheme queries, clipboard reads and unknown sequences have no
        // UI yet (PLAN §4b, §8.1, §9); they are dropped here, in one place.
        _ => return None,
    })
}

/// Ends the program: hangs up its process group, polls for the exit for the grace period, then
/// kills it. `None` when the status is no longer known.
fn reap(io: &mut Io) -> Option<ChildExit> {
    // Already gone is fine: the poll below sees the exit either way.
    let _ = io.pty.hang_up();
    let mut waited = Duration::ZERO;
    loop {
        if let Ok(Some(exit)) = io.pty.try_exit() {
            return Some(exit);
        }
        match reap_step(waited, GRACE) {
            ReapStep::Poll => {
                std::thread::sleep(POLL);
                waited += POLL;
            }
            // `kill` reaps the child itself; `None` when the status is no longer known.
            ReapStep::Kill => return io.pty.kill().ok(),
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
