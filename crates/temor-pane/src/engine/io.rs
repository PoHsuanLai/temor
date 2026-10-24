//! The pane's I/O thread (PLAN §4b). It reads the PTY and writes the terminal under the pane's
//! lock; the terminal's replies go straight back to the PTY and its other events go to the
//! window. Output is coalesced: the window hears `Dirty` once until it has repainted, so a flood
//! of output costs one event, not one per read. It also reaps the program: when the program ends
//! or the pane is shut down, the terminal hangs up, the process group gets the grace period, and
//! then it is killed.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use temor_pty::{ChildExit, Pty, ReadOutcome, Ready};
use temor_transfer::Consent;
use temor_transfer::kitty::Offered;
use temor_vt::{RenderHold, Snapshot, Truncation, UnknownKind, VtEvent};

use super::event::PaneEvent;
use super::fed_io::{FedSetup, pump_fed};
use super::outbox::Outbox;
use super::poison::lock;
use super::reap::{GRACE, POLL, ReapStep, reap_step};
use super::sink::EventSink;
use super::tap::ByteTap;
use super::term::Term;
use super::watch::ForegroundWatch;
use super::xfer::{OfferId, Reaction, Transfers};

/// What the payload of a shell state report starts with (`temor_shell::report::OSC_NUMBER`).
const REPORT_HEAD: &[u8] = b"7503;";

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
    /// Hears the raw output before the terminal parses it.
    pub tap: Option<Arc<dyn ByteTap>>,
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
            Ok(ReadOutcome::Data(n)) => {
                if let Some(tap) = io.tap.as_ref() {
                    tap.bytes(&buf[..n]);
                }
                deliver(&mut io, &buf[..n]);
            }
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

pub(super) fn decide(io: &mut Io, offer: OfferId, consent: Consent) {
    let reaction = io.transfers.as_mut().map(|t| t.decide(offer, consent));
    carry_out(io, reaction);
}

pub(super) fn serve(io: &mut Io, offer: OfferId, consent: Consent, shelf: &[Offered]) {
    let reaction = io
        .transfers
        .as_mut()
        .map(|t| t.serve(offer, consent, shelf));
    carry_out(io, reaction);
}

/// Feeds program output to the terminal, sends its replies to the PTY and its events on.
pub(super) fn deliver(io: &mut Io, bytes: &[u8]) {
    let read = Instant::now();
    let output = lock(&io.term).write(bytes);
    for reply in output.replies {
        let _ = io.outbox.push_reply(&reply);
    }
    for event in output.events {
        if let VtEvent::Unknown {
            kind: UnknownKind::Osc,
            payload,
            truncation: Truncation::Complete,
        } = &event
            && let Some(report) = payload.strip_prefix(REPORT_HEAD)
        {
            if let Ok(report) = std::str::from_utf8(report) {
                send(io, PaneEvent::ShellReport(report.to_owned()));
            }
            continue;
        }
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
        if let Some(event) = map_event(event, read) {
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
pub(super) fn carry_out(io: &mut Io, reaction: Option<Reaction>) {
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

/// The window's view of a terminal event, read at `read`; events the window does not use yet are
/// dropped.
pub(super) fn map_event(event: VtEvent, read: Instant) -> Option<PaneEvent> {
    Some(match event {
        VtEvent::Title(title) => PaneEvent::Title(title),
        VtEvent::Bell => PaneEvent::Bell,
        VtEvent::Cwd(url) => PaneEvent::Cwd(url),
        VtEvent::Clipboard { write } => PaneEvent::Clipboard(write),
        VtEvent::ClipboardRead { read } => PaneEvent::ClipboardRead(read),
        VtEvent::Notification(note) => PaneEvent::Notification(note),
        VtEvent::MarkBegin { seq } => PaneEvent::MarkBegin { seq },
        VtEvent::Prompt(mark) => PaneEvent::Prompt(mark),
        VtEvent::Anchor { seq, anchor } => PaneEvent::Anchor {
            seq,
            anchor,
            at: read,
        },
        VtEvent::Progress(progress) => PaneEvent::Progress(progress),
        VtEvent::Reset => PaneEvent::Reset,
        VtEvent::RenderHold(RenderHold::Held) => PaneEvent::RenderHold(super::event::Hold::Held),
        VtEvent::RenderHold(RenderHold::Released) => {
            PaneEvent::RenderHold(super::event::Hold::Released)
        }
        VtEvent::ProgramStatus(status) => PaneEvent::Program(status),
        // Colour-scheme queries and unknown sequences have no UI yet
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

pub(super) fn send(io: &Io, event: PaneEvent) {
    // A closed channel means the window is gone; the sink drops the event.
    io.events.send(event);
}
