//! The window's side of a pane: the terminal to read and encode under its lock, the input to
//! send, and the resize. Dropping the handle shuts the pane down (the I/O thread reaps the
//! program).

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

/// How long a resize waits for the I/O thread to tell the program (it polls every 20 ms).
const RESIZE_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

/// A running pane, as the window holds it.
#[derive(Debug)]
pub struct PaneHandle {
    term: Arc<Mutex<Term>>,
    outbox: Arc<Outbox>,
    commands: Sender<Command>,
    events: UnboundedSender<PaneEvent>,
}

impl PaneHandle {
    pub(super) fn new(
        term: Arc<Mutex<Term>>,
        outbox: Arc<Outbox>,
        commands: Sender<Command>,
        events: UnboundedSender<PaneEvent>,
    ) -> Self {
        Self {
            term,
            outbox,
            commands,
            events,
        }
    }

    /// Runs `f` with the terminal locked (PLAN §4b: the lock is held only for the read).
    pub fn with_term<T>(&self, f: impl FnOnce(&mut Term) -> T) -> T {
        f(&mut lock_term(&self.term))
    }

    /// Sends program input (keys, paste, a mouse report) in order.
    pub fn send(&self, bytes: &[u8]) {
        if !bytes.is_empty() {
            self.outbox.push(bytes);
        }
    }

    /// Encodes a key with the terminal's current modes and sends it.
    pub fn key(&self, key: &KeyInput) -> Result<(), UiError> {
        let bytes = self.with_term(|term| term.encode_key(key))?;
        self.send(&bytes);
        Ok(())
    }

    /// Changes the grid: the terminal reflows, the program is told its new size, and the
    /// window repaints.
    pub fn resize(&self, grid: GridSize, cell: CellPx) -> Result<(), UiError> {
        let output = self.with_term(|term| term.resize(grid, cell))?;
        for reply in output.replies {
            self.outbox.push(&reply);
        }
        for event in output.events.into_iter().filter_map(map_event) {
            let _ = self.events.send(event);
        }
        let _ = self.events.send(PaneEvent::Dirty);
        // The program is told its new size before this returns, so its next output is at the new
        // size. A closed I/O thread has already reaped the program; there is nothing to wait for.
        let (done, applied) = std::sync::mpsc::sync_channel(1);
        if self
            .commands
            .send(Command::Resize {
                size: pty_size(grid, cell),
                done,
            })
            .is_ok()
        {
            let _ = applied.recv_timeout(RESIZE_WAIT);
        }
        Ok(())
    }
}

impl Drop for PaneHandle {
    fn drop(&mut self) {
        // The thread may be gone already (the program exited); then there is nothing to stop.
        let _ = self.commands.send(Command::Shutdown);
    }
}
