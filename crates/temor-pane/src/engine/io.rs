//! The pane's I/O thread (PLAN §4b). It reads the PTY and writes the terminal under the pane's
//! lock; the terminal's replies go straight back to the PTY and its other events go to the
//! window. Output is coalesced: the window hears `Dirty` once until it has repainted, so a flood
//! of output costs one event, not one per read. It also reaps the program: when the program ends
//! or the pane is shut down, the terminal hangs up, the process group gets the grace period, and
//! then it is killed.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use temor_pty::{ChildExit, Pty, ReadOutcome, Ready};
use temor_render::Palette;
use temor_transfer::Consent;
use temor_transfer::kitty::Offered;
use temor_vt::{CellPx, CursorShape, LineCount, RenderHold, Snapshot, VtEvent};

use super::event::PaneEvent;
use super::outbox::Outbox;
use super::reap::{GRACE, POLL, ReapStep, reap_step};
use super::sink::EventSink;
use super::term::Term;
use super::watch::ForegroundWatch;
use super::xfer::{OfferId, Reaction, Transfers};

/// What the window asks of the I/O thread.
#[derive(Debug)]
pub(super) enum Command {
    /// The window is gone: hang up and reap.
    Shutdown,
    /// The person's answer to a program's offer of files.
    Decide(OfferId, Consent),
    /// The person's answer to a program's request for a file, with what the shelf can hand out.
    Serve(OfferId, Consent, Vec<Offered>),
    /// Output from the far end of a fed pane.
    Feed(Vec<u8>),
    /// A fed pane's screen starts over from this snapshot (the link came back).
    Restore(Snapshot),
    /// A fed pane's program ended.
    End(Option<ChildExit>),
}

/// What a fed pane needs to rebuild its terminal from a snapshot.
#[derive(Debug, Clone)]
pub(super) struct FedSetup {
    pub cell: CellPx,
    pub scrollback: LineCount,
    pub cursor: CursorShape,
    pub palette: Palette,
}

/// Whether a fed pane's loop goes on.
enum Flow {
    Go,
    Stop,
}

/// How long a fed pane waits for a command before looking at its transfers again.
const FED_POLL: Duration = Duration::from_millis(100);

/// Everything the thread owns or shares.
pub(super) struct Io {
    pub term: Arc<Mutex<Term>>,
    pub outbox: Arc<Outbox>,
    /// The program's terminal; `None` for a pane fed from elsewhere.
    pub pty: Option<Pty>,
    /// Set for a pane fed from elsewhere.
    pub fed: Option<FedSetup>,
    pub commands: Receiver<Command>,
    pub events: Arc<dyn EventSink>,
    /// Set while a `Dirty` is in flight; cleared by the window when it repaints.
    pub dirty: Arc<AtomicBool>,
    /// The last title sent, so a program that sets the same title in a loop costs nothing.
    pub last_title: Option<String>,
    /// Files the program sends or asks for; `None` when the pane takes none.
    pub transfers: Option<Transfers>,
    /// The foreground process's name, looked at about once a second.
    pub watch: ForegroundWatch,
}

/// The pane's I/O loop, made by [`start`](super::start). `Send`: it moves to the thread the caller
/// starts it on. Dropped without [`run`](Self::run), it still hangs up and reaps the program.
pub struct PaneRunner {
    io: Option<Io>,
}

impl std::fmt::Debug for PaneRunner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaneRunner").finish_non_exhaustive()
    }
}

impl PaneRunner {
    pub(super) fn new(io: Io) -> Self {
        Self { io: Some(io) }
    }

    /// Runs until the program is reaped (or the pane is dropped). Blocks: call it on the pane's
    /// own thread, which the caller starts.
    pub fn run(mut self) {
        if let Some(io) = self.io.take() {
            if io.fed.is_some() {
                pump_fed(io);
            } else {
                pump(io);
            }
        }
    }
}

impl Drop for PaneRunner {
    fn drop(&mut self) {
        // Never run (the thread could not start, say): the program is still hung up and reaped,
        // so it does not outlive the pane as a zombie.
        if let Some(mut io) = self.io.take() {
            finish(&mut io);
        }
    }
}

/// Runs until the program is reaped. Called on the pane's own thread.
fn pump(mut io: Io) {
    let mut buf = vec![0u8; 64 * 1024];
    let outbox = Arc::clone(&io.outbox);
    let Some(port) = outbox.pty_port() else {
        return finish(&mut io);
    };
    loop {
        while let Ok(command) = io.commands.try_recv() {
            match command {
                Command::Shutdown => return finish(&mut io),
                Command::Decide(offer, consent) => decide(&mut io, offer, consent),
                Command::Serve(offer, consent, shelf) => serve(&mut io, offer, consent, &shelf),
                // Only a fed pane takes these.
                Command::Feed(_) | Command::Restore(_) | Command::End(_) => {}
            }
        }
        // A file on its way to the program goes a few frames at a time, once the last are taken.
        if !io.outbox.is_waiting() && io.transfers.as_ref().is_some_and(|t| t.sending()) {
            let reaction = io.transfers.as_mut().map(Transfers::pump_sends);
            carry_out(&mut io, reaction);
        }
        if let Some(pty) = io.pty.as_ref()
            && let Some(event) = io.watch.poll(pty)
        {
            send(&io, event);
        }
        let interest = if io.outbox.is_waiting() {
            temor_pty::Interest::ReadWrite
        } else {
            temor_pty::Interest::Read
        };
        // A file being sent should not wait out a whole poll between its frames.
        let poll = match io.transfers.as_ref().is_some_and(|t| t.sending()) {
            true => Duration::from_millis(1),
            false => POLL,
        };
        let ready = match port.wait(interest, Some(poll)) {
            Ok(ready) => ready,
            Err(_) => return finish(&mut io),
        };
        if ready.write == Ready::Now {
            io.outbox.flush();
        }
        if ready.read != Ready::Now {
            continue;
        }
        match port.read(&mut buf) {
            Ok(ReadOutcome::Data(n)) => deliver(&mut io, &buf[..n]),
            Ok(ReadOutcome::WouldBlock) => {}
            // Every holder of the terminal has closed it: the program is done.
            Ok(ReadOutcome::Closed) | Err(_) => return finish(&mut io),
        }
    }
}

/// Reaps the program and tells the window how it ended.
fn finish(io: &mut Io) {
    let aborted = io.transfers.as_mut().map(Transfers::abort_all);
    carry_out(io, aborted);
    let exit = reap(io);
    send(io, PaneEvent::Exited(exit));
}

fn decide(io: &mut Io, offer: OfferId, consent: Consent) {
    let reaction = io.transfers.as_mut().map(|t| t.decide(offer, consent));
    carry_out(io, reaction);
}

fn serve(io: &mut Io, offer: OfferId, consent: Consent, shelf: &[Offered]) {
    let reaction = io
        .transfers
        .as_mut()
        .map(|t| t.serve(offer, consent, shelf));
    carry_out(io, reaction);
}

/// The loop of a pane fed from elsewhere: output arrives as commands, nothing is polled.
fn pump_fed(mut io: Io) {
    loop {
        let sending = io.transfers.as_ref().is_some_and(|t| t.sending());
        let wait = match sending {
            true => Duration::from_millis(1),
            false => FED_POLL,
        };
        match io.commands.recv_timeout(wait) {
            Ok(command) => {
                if let Flow::Stop = fed_command(&mut io, command) {
                    return;
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            // Every sender is gone, the handle included: nobody is left to show it to.
            Err(RecvTimeoutError::Disconnected) => return abort_transfers(&mut io),
        }
        while let Ok(command) = io.commands.try_recv() {
            if let Flow::Stop = fed_command(&mut io, command) {
                return;
            }
        }
        if !io.outbox.is_waiting() && io.transfers.as_ref().is_some_and(|t| t.sending()) {
            let reaction = io.transfers.as_mut().map(Transfers::pump_sends);
            carry_out(&mut io, reaction);
        }
    }
}

fn fed_command(io: &mut Io, command: Command) -> Flow {
    match command {
        Command::Shutdown => {
            abort_transfers(io);
            return Flow::Stop;
        }
        Command::Decide(offer, consent) => decide(io, offer, consent),
        Command::Serve(offer, consent, shelf) => serve(io, offer, consent, &shelf),
        Command::Feed(bytes) => deliver(io, &bytes),
        Command::Restore(snapshot) => restore(io, &snapshot),
        Command::End(exit) => {
            abort_transfers(io);
            send(io, PaneEvent::Exited(exit));
            return Flow::Stop;
        }
    }
    Flow::Go
}

fn abort_transfers(io: &mut Io) {
    let aborted = io.transfers.as_mut().map(Transfers::abort_all);
    carry_out(io, aborted);
}

/// Replaces a fed pane's terminal with one that begins as `snapshot`, and tells the window the
/// screen was reset (what it knew of the old one is gone).
fn restore(io: &mut Io, snapshot: &Snapshot) {
    let Some(setup) = io.fed.clone() else {
        return;
    };
    let restored = Term::restored(
        snapshot,
        setup.cell,
        setup.scrollback,
        setup.cursor,
        &setup.palette,
    );
    // A snapshot that cannot be read leaves the old screen; the live output still follows.
    if let Ok(term) = restored {
        *lock_term(&io.term) = term;
        send(io, PaneEvent::Reset);
        signal_dirty(io);
    }
}

/// Feeds program output to the terminal, sends its replies to the PTY and its events on.
fn deliver(io: &mut Io, bytes: &[u8]) {
    let output = lock_term(&io.term).write(bytes);
    for reply in output.replies {
        let _ = io.outbox.push_reply(&reply);
    }
    for event in output.events {
        if let VtEvent::Unknown {
            kind,
            payload,
            truncation,
        } = &event
            && let Some(transfers) = io.transfers.as_mut()
        {
            let reaction = transfers.on_unknown(*kind, payload, *truncation);
            carry_out(io, Some(reaction));
            continue;
        }
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

/// Sends a transfer step's sequences to the program and its events to the window.
fn carry_out(io: &mut Io, reaction: Option<Reaction>) {
    let Some(reaction) = reaction else {
        return;
    };
    for bytes in reaction.replies {
        let _ = io.outbox.push_reply(&bytes);
    }
    for event in reaction.events {
        send(io, PaneEvent::Transfer(event));
    }
}

/// Tells the window the screen changed, unless it has not repainted the last such signal yet.
pub(super) fn signal_dirty(io: &Io) {
    if !io.dirty.swap(true, Ordering::AcqRel) {
        io.events.send(PaneEvent::Dirty);
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
        VtEvent::MarkBegin { seq } => PaneEvent::MarkBegin { seq },
        VtEvent::Prompt(mark) => PaneEvent::Prompt(mark),
        VtEvent::Anchor { seq, anchor } => PaneEvent::Anchor { seq, anchor },
        VtEvent::Progress(progress) => PaneEvent::Progress(progress),
        VtEvent::Reset => PaneEvent::Reset,
        VtEvent::RenderHold(RenderHold::Held) => PaneEvent::RenderHold(super::event::Hold::Held),
        VtEvent::RenderHold(RenderHold::Released) => {
            PaneEvent::RenderHold(super::event::Hold::Released)
        }
        VtEvent::ProgramStatus(status) => PaneEvent::Program(status),
        // Colour-scheme queries, clipboard reads and unknown sequences have no UI yet
        // (PLAN §4b, §9); they are dropped here, in one place.
        _ => return None,
    })
}

/// Ends the program: hangs up its process group, polls for the exit for the grace period, then
/// kills it. `None` when the status is no longer known.
fn reap(io: &mut Io) -> Option<ChildExit> {
    let pty = io.pty.as_mut()?;
    // Already gone is fine: the poll below sees the exit either way.
    let _ = pty.hang_up();
    let mut waited = Duration::ZERO;
    loop {
        if let Ok(Some(exit)) = pty.try_exit() {
            return Some(exit);
        }
        match reap_step(waited, GRACE) {
            ReapStep::Poll => {
                std::thread::sleep(POLL);
                waited += POLL;
            }
            // `kill` reaps the child itself; `None` when the status is no longer known.
            ReapStep::Kill => return pty.kill().ok(),
        }
    }
}

fn send(io: &Io, event: PaneEvent) {
    // A closed channel means the window is gone; the sink drops the event.
    io.events.send(event);
}

/// Locks a pane's terminal. A poisoned lock still holds the terminal, so the pane keeps going.
pub(super) fn lock_term(term: &Mutex<Term>) -> MutexGuard<'_, Term> {
    term.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}
