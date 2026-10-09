//! The window's side of a pane: the terminal to read and encode under its lock, the input to
//! send, and the resize. Dropping the handle shuts the pane down (the I/O thread reaps the
//! program).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use temor_vt::{CellPx, GridSize, KeyInput};
use tokio::sync::mpsc::UnboundedSender;

use super::event::PaneEvent;
use super::io::{Command, lock_term, map_event};
use super::outbox::Outbox;
use super::spawn::pty_size;
use super::term::Term;
use crate::error::UiError;

/// A running pane, as the window holds it.
#[derive(Debug)]
pub struct PaneHandle {
    term: Arc<Mutex<Term>>,
    outbox: Arc<Outbox>,
    commands: Sender<Command>,
    events: UnboundedSender<PaneEvent>,
    dirty: Arc<AtomicBool>,
}

impl PaneHandle {
    pub(super) fn new(
        term: Arc<Mutex<Term>>,
        outbox: Arc<Outbox>,
        commands: Sender<Command>,
        events: UnboundedSender<PaneEvent>,
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
        f(&mut lock_term(&self.term))
    }

    /// Sends program input (keys, paste, a mouse report) in order.
    pub fn send(&self, bytes: &[u8]) {
        if !bytes.is_empty() {
            self.outbox.push_input(bytes);
        }
    }

    /// Encodes a key with the terminal's current modes and sends it.
    pub fn key(&self, key: &KeyInput) -> Result<(), UiError> {
        let bytes = self.with_term(|term| term.encode_key(key))?;
        self.send(&bytes);
        Ok(())
    }

    /// Changes the grid. The program is told the new size first (by the I/O thread, which does
    /// not block this call); then the terminal reflows and the window is asked to repaint.
    pub fn resize(&self, grid: GridSize, cell: CellPx) -> Result<(), UiError> {
        let _ = self.commands.send(Command::Resize(pty_size(grid, cell)));
        let output = self.with_term(|term| term.resize(grid, cell))?;
        for reply in output.replies {
            let _ = self.outbox.push_reply(&reply);
        }
        for event in output.events.into_iter().filter_map(map_event) {
            let _ = self.events.send(event);
        }
        self.request_repaint();
        Ok(())
    }

    /// Asks the window for a repaint, unless one is already on its way.
    pub fn request_repaint(&self) {
        if !self.dirty.swap(true, Ordering::AcqRel) {
            let _ = self.events.send(PaneEvent::Dirty);
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
