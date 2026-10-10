//! The bytes waiting to go to a session's program, in order. The person's input is never
//! dropped: it queues in full and goes out as the terminal accepts it. The terminal's own replies
//! are capped, so a program that never reads cannot grow the queue without bound.

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard};

use temor_pty::{PtyError, PtyIo, WriteOutcome};

/// The most reply bytes that may wait (1 MiB).
const REPLY_CAP: usize = 1 << 20;

/// Whether bytes are still waiting for the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Backlog {
    Empty,
    Waiting,
}

/// A session's input queue and the terminal's writing end. `Send` and `Sync`.
#[derive(Debug)]
pub(super) struct Input {
    io: PtyIo,
    queue: Mutex<VecDeque<u8>>,
}

impl Input {
    pub(super) fn new(io: PtyIo) -> Self {
        Self {
            io,
            queue: Mutex::new(VecDeque::new()),
        }
    }

    /// Queues the person's input and writes what the terminal takes.
    pub(super) fn send(&self, bytes: &[u8]) {
        let mut queue = self.lock();
        queue.extend(bytes);
        self.drain(&mut queue);
    }

    /// Queues a reply from the screen state; dropped when too many are waiting.
    pub(super) fn reply(&self, bytes: &[u8]) {
        let mut queue = self.lock();
        if queue.len() + bytes.len() <= REPLY_CAP {
            queue.extend(bytes);
            self.drain(&mut queue);
        }
    }

    /// Writes what the terminal takes now.
    pub(super) fn flush(&self) -> Backlog {
        let mut queue = self.lock();
        self.drain(&mut queue);
        backlog(&queue)
    }

    pub(super) fn backlog(&self) -> Backlog {
        backlog(&self.lock())
    }

    /// Tells the program its new size.
    pub(super) fn resize(&self, size: temor_pty::GridSize) -> Result<(), PtyError> {
        self.io.resize(size)
    }

    fn drain(&self, queue: &mut VecDeque<u8>) {
        while !queue.is_empty() {
            match self.io.write(queue.make_contiguous()) {
                Ok(WriteOutcome::Wrote(written)) => {
                    queue.drain(..written);
                }
                Ok(WriteOutcome::WouldBlock) => return,
                // The program is gone; its pump notices on the read side.
                Ok(WriteOutcome::Closed) | Err(_) => {
                    queue.clear();
                    return;
                }
            }
        }
    }

    fn lock(&self) -> MutexGuard<'_, VecDeque<u8>> {
        self.queue
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn backlog(queue: &VecDeque<u8>) -> Backlog {
    if queue.is_empty() {
        Backlog::Empty
    } else {
        Backlog::Waiting
    }
}
