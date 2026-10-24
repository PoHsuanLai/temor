//! The window's side of a pane: the terminal to read and encode under its lock, the input to
//! send, and the resize. Dropping the handle shuts the pane down (the I/O thread reaps the
//! program).

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use temor_transfer::Consent;
use temor_transfer::kitty::Offered;
use temor_vt::{CellPx, GridSize, KeyInput};

use super::event::PaneEvent;
use super::io::{Command, map_event};
use super::outbox::Outbox;
use super::poison::lock;
use super::sink::EventSink;
use super::term::Term;
use super::xfer::OfferId;
use crate::error::PaneError;

/// A running pane, as the window holds it. `Send` and `Sync`: the window may hold it on any thread;
/// the terminal is locked for each read or encode.
pub struct PaneHandle {
    term: Arc<Mutex<Term>>,
    outbox: Arc<Outbox>,
    commands: Sender<Command>,
    events: Arc<dyn EventSink>,
    dirty: Arc<AtomicBool>,
}

impl fmt::Debug for PaneHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PaneHandle").finish_non_exhaustive()
    }
}

impl PaneHandle {
    pub(super) fn new(
        term: Arc<Mutex<Term>>,
        outbox: Arc<Outbox>,
        commands: Sender<Command>,
        events: Arc<dyn EventSink>,
        dirty: Arc<AtomicBool>,
    ) -> Self {
        Self {
            term,
            outbox,
            commands,
            events,
            dirty,
        }
    }

    /// Runs `f` with the terminal locked (PLAN §4b: the lock is held only for the read).
    pub fn with_term<T>(&self, f: impl FnOnce(&mut Term) -> T) -> T {
        f(&mut lock(&self.term))
    }

    /// Sends program input (keys, paste, a mouse report) in order.
    pub fn send(&self, bytes: &[u8]) {
        if !bytes.is_empty() {
            self.outbox.push_input(bytes);
        }
    }

    /// Encodes a key with the terminal's current modes and sends it.
    pub fn key(&self, key: &KeyInput) -> Result<(), PaneError> {
        let bytes = self.with_term(|term| term.encode_key(key))?;
        self.send(&bytes);
        Ok(())
    }

    /// Changes the grid. The terminal reflows and the program's size changes inside one hold of
    /// the pane's lock, so the I/O thread never parses output against a half-changed pane. Output
    /// the program wrote before the size change but read from the PTY just before it may still be
    /// parsed at the new size; that one read is the same window every terminal has on SIGWINCH.
    ///
    /// Draining the PTY first would not close it. Only the I/O thread may read the PTY (a second
    /// reader would reorder bytes against the read in flight), so a drain means asking that thread
    /// and waiting for it, which blocks the window on a thread that may be busy with a flood of
    /// output; and whatever the program writes between the drain and the size change is in the
    /// same position. The program redraws on SIGWINCH anyway, so the one mis-parsed read is
    /// repainted a moment later. Closed (FOLLOWUPS, "Output just before a resize").
    pub fn resize(&self, grid: GridSize, cell: CellPx) -> Result<(), PaneError> {
        let output = self.with_term(|term| {
            let output = term.resize(grid, cell)?;
            // A failed size change leaves the program at its old size; the grid is still right.
            self.outbox.resize(grid, cell);
            Ok::<_, PaneError>(output)
        })?;
        for reply in output.replies {
            let _ = self.outbox.push_reply(&reply);
        }
        let read = Instant::now();
        for event in output.events.into_iter().filter_map(|e| map_event(e, read)) {
            self.events.send(event);
        }
        self.request_repaint();
        Ok(())
    }

    /// The person's answer to a program's offer of files. The pane acts on it at its next turn.
    pub fn decide(&self, offer: OfferId, consent: Consent) {
        // The thread may be gone already (the program exited); then nothing waits for the answer.
        let _ = self.commands.send(Command::Decide(offer, consent));
    }

    /// The person's answer to a program's request for a file. `shelf` is what could be handed out,
    /// newest first.
    pub fn serve(&self, offer: OfferId, consent: Consent, shelf: Vec<Offered>) {
        let _ = self.commands.send(Command::Serve(offer, consent, shelf));
    }

    /// Asks the window for a repaint, unless one is already on its way.
    pub fn request_repaint(&self) {
        if !self.dirty.swap(true, Ordering::AcqRel) {
            self.events.send(PaneEvent::Dirty);
        }
    }

    /// The window has repainted: the next change may signal again.
    pub fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::Release);
    }
}

impl Drop for PaneHandle {
    fn drop(&mut self) {
        // The thread may be gone already (the program exited); then there is nothing to stop.
        let _ = self.commands.send(Command::Shutdown);
    }
}
