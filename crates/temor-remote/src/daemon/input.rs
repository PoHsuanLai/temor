//! The bytes waiting to go to a session's program, in order. The person's input is never
//! dropped: it queues in full and goes out as the terminal accepts it. The terminal's own replies
//! are capped, so a program that never reads cannot grow the queue without bound.

use std::collections::VecDeque;

use temor_pty::{PtyError, PtyIo, WriteOutcome};

/// The most reply bytes that may wait (1 MiB).
const REPLY_CAP: usize = 1 << 20;

/// A session's input queue and the terminal's writing end, owned by its pump.
#[derive(Debug)]
pub(super) struct Input {
    io: PtyIo,
    queue: VecDeque<u8>,
}

impl Input {
    pub(super) fn new(io: PtyIo) -> Self {
        Self {
            io,
            queue: VecDeque::new(),
        }
    }

    /// Queues the person's input and writes what the terminal takes.
    pub(super) fn send(&mut self, bytes: &[u8]) {
        self.queue.extend(bytes);
        self.flush();
    }

    /// Queues a reply from the screen state; dropped when too many are waiting.
    pub(super) fn reply(&mut self, bytes: &[u8]) {
        if self.queue.len() + bytes.len() <= REPLY_CAP {
            self.send(bytes);
        }
    }

    /// Whether bytes are waiting for the terminal to take them.
    pub(super) fn backlog(&self) -> bool {
        !self.queue.is_empty()
    }

    /// Tells the program its new size.
    pub(super) fn resize(&self, size: temor_pty::GridSize) -> Result<(), PtyError> {
        self.io.resize(size)
    }

    /// Writes what the terminal takes now.
    pub(super) fn flush(&mut self) {
        while !self.queue.is_empty() {
            match self.io.write(self.queue.make_contiguous()) {
                Ok(WriteOutcome::Wrote(written)) => {
                    self.queue.drain(..written);
                }
                Ok(WriteOutcome::WouldBlock) => return,
                // The program is gone; its pump notices on the read side.
                Ok(WriteOutcome::Closed) | Err(_) => {
                    self.queue.clear();
                    return;
                }
            }
        }
    }
}
