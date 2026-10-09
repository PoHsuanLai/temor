//! The bytes waiting to go to the program. Every write, from the window's keys and paste and
//! from the terminal's own replies, goes through one queue under one lock, so their order is
//! kept. The write is tried at once; what the terminal does not take waits for the I/O thread.

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard};

use temor_pty::{PtyIo, WriteOutcome};

/// The queue and the terminal's write end.
#[derive(Debug)]
pub struct Outbox {
    queue: Mutex<VecDeque<u8>>,
    port: PtyIo,
}

impl Outbox {
    pub fn new(port: PtyIo) -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            port,
        }
    }

    /// The write end, for the I/O thread's reads and waits.
    pub fn port(&self) -> &PtyIo {
        &self.port
    }

    /// Queues `bytes` after what is waiting and writes as much as the terminal takes.
    pub fn push(&self, bytes: &[u8]) {
        let mut queue = self.lock();
        queue.extend(bytes.iter().copied());
        flush(&self.port, &mut queue);
    }

    /// Writes what is waiting, as far as the terminal takes it now. Returns whether nothing
    /// is left.
    pub fn flush(&self) -> bool {
        let mut queue = self.lock();
        flush(&self.port, &mut queue);
        queue.is_empty()
    }

    /// Whether bytes are waiting for the terminal to take them.
    pub fn is_waiting(&self) -> bool {
        !self.lock().is_empty()
    }

    fn lock(&self) -> MutexGuard<'_, VecDeque<u8>> {
        // A poisoned queue still holds the bytes; the pane keeps going rather than dropping them.
        self.queue
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn flush(port: &PtyIo, queue: &mut VecDeque<u8>) {
    while !queue.is_empty() {
        let chunk: Vec<u8> = queue.iter().copied().take(4096).collect();
        match port.write(&chunk) {
            Ok(WriteOutcome::Wrote(n)) if n > 0 => {
                queue.drain(..n.min(queue.len()));
            }
            Ok(WriteOutcome::Closed) => {
                queue.clear();
                return;
            }
            // Would block, wrote nothing, or failed: the I/O thread retries or reaps.
            _ => return,
        }
    }
}
