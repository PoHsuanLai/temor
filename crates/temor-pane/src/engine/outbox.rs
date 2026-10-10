//! The bytes waiting to go to the program, in the order they were made. The person's input and
//! pastes are never dropped: they queue in full and go out in chunks as the terminal accepts
//! them (back-pressure). The terminal's own replies are the only bytes with a cap; beyond
//! [`REPLY_CAP`] a reply is dropped and counted, never a paste (PLAN §4b).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

use temor_pty::{PtyIo, WriteOutcome};
use temor_vt::{CellPx, GridSize};

use super::fed::PaneLink;
use super::spawn::pty_size;

/// The most reply bytes that may wait for the program (1 MiB).
pub const REPLY_CAP: usize = 1 << 20;

/// The most bytes handed to the terminal in one write.
const CHUNK: usize = 4096;

/// One queued write: its bytes, how many are gone, and whether it is a reply (capped).
#[derive(Debug)]
struct Segment {
    bytes: Vec<u8>,
    sent: usize,
    reply: bool,
}

/// The queue itself, without the terminal: the pure part, tested on its own.
#[derive(Debug, Default)]
pub struct Queue {
    segments: VecDeque<Segment>,
    reply_bytes: usize,
}

impl Queue {
    /// Queues the person's input or a paste. Never refused and never shortened.
    pub fn push_input(&mut self, bytes: &[u8]) {
        if !bytes.is_empty() {
            self.segments.push_back(Segment {
                bytes: bytes.to_vec(),
                sent: 0,
                reply: false,
            });
        }
    }

    /// Queues a reply from the terminal. Returns how many of its bytes were dropped because the
    /// reply queue is full (all of them, or none).
    pub fn push_reply(&mut self, bytes: &[u8]) -> usize {
        if bytes.is_empty() {
            return 0;
        }
        if self.reply_bytes + bytes.len() > REPLY_CAP {
            return bytes.len();
        }
        self.reply_bytes += bytes.len();
        self.segments.push_back(Segment {
            bytes: bytes.to_vec(),
            sent: 0,
            reply: true,
        });
        0
    }

    /// The next bytes to write: at most one chunk, from the oldest segment.
    pub fn next_chunk(&self) -> Option<&[u8]> {
        let front = self.segments.front()?;
        let end = front.bytes.len().min(front.sent + CHUNK);
        Some(&front.bytes[front.sent..end])
    }

    /// The terminal took `n` of the bytes `next_chunk` returned.
    pub fn advance(&mut self, n: usize) {
        let Some(front) = self.segments.front_mut() else {
            return;
        };
        front.sent = (front.sent + n).min(front.bytes.len());
        if front.sent == front.bytes.len() {
            let done = self.segments.pop_front();
            if let Some(done) = done.filter(|done| done.reply) {
                self.reply_bytes -= done.bytes.len();
            }
        }
    }

    /// Whether nothing waits.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// Forgets everything (the program's end: nobody will read it).
    pub fn clear(&mut self) {
        self.segments.clear();
        self.reply_bytes = 0;
    }
}

/// Where the queued bytes go: a pseudo-terminal, or a link to a program somewhere else.
#[derive(Debug)]
enum Port {
    Pty(PtyIo),
    Link(Arc<dyn PaneLink>),
}

/// The queue and the program's write end.
#[derive(Debug)]
pub struct Outbox {
    queue: Mutex<Queue>,
    port: Port,
}

impl Outbox {
    pub fn new(port: PtyIo) -> Self {
        Self {
            queue: Mutex::new(Queue::default()),
            port: Port::Pty(port),
        }
    }

    /// An outbox whose bytes go to `link` (a pane fed from elsewhere).
    pub fn linked(link: Arc<dyn PaneLink>) -> Self {
        Self {
            queue: Mutex::new(Queue::default()),
            port: Port::Link(link),
        }
    }

    /// The pseudo-terminal, for the I/O thread's reads and waits; `None` for a linked pane.
    pub fn pty_port(&self) -> Option<&PtyIo> {
        match &self.port {
            Port::Pty(port) => Some(port),
            Port::Link(_) => None,
        }
    }

    /// Tells the program the grid changed. A failed size change leaves it at its old size.
    pub fn resize(&self, grid: GridSize, cell: CellPx) {
        match &self.port {
            Port::Pty(port) => {
                let _ = port.resize(pty_size(grid, cell));
            }
            Port::Link(link) => link.resize(grid, cell),
        }
    }

    /// Queues the person's input or a paste after what is waiting, and writes what the terminal
    /// takes now.
    pub fn push_input(&self, bytes: &[u8]) {
        let mut queue = self.lock();
        queue.push_input(bytes);
        flush(&self.port, &mut queue);
    }

    /// Queues a terminal reply; returns how many of its bytes were dropped (see [`Queue`]).
    pub fn push_reply(&self, bytes: &[u8]) -> usize {
        let mut queue = self.lock();
        let dropped = queue.push_reply(bytes);
        flush(&self.port, &mut queue);
        dropped
    }

    /// Writes what is waiting, as far as the terminal takes it now. Returns whether nothing is
    /// left.
    pub fn flush(&self) -> bool {
        let mut queue = self.lock();
        flush(&self.port, &mut queue);
        queue.is_empty()
    }

    /// Whether bytes are waiting for the terminal to take them.
    pub fn is_waiting(&self) -> bool {
        !self.lock().is_empty()
    }

    fn lock(&self) -> MutexGuard<'_, Queue> {
        // A poisoned queue still holds the bytes; the pane keeps going rather than dropping them.
        self.queue
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn flush(port: &Port, queue: &mut Queue) {
    match port {
        Port::Pty(port) => flush_pty(port, queue),
        Port::Link(link) => flush_link(link.as_ref(), queue),
    }
}

/// A link takes every chunk whole; a link that fails is gone, so what waits is forgotten.
fn flush_link(link: &dyn PaneLink, queue: &mut Queue) {
    while let Some(chunk) = queue.next_chunk() {
        let n = chunk.len();
        match link.send(chunk) {
            Ok(()) => queue.advance(n),
            Err(_) => {
                queue.clear();
                return;
            }
        }
    }
}

fn flush_pty(port: &PtyIo, queue: &mut Queue) {
    while let Some(chunk) = queue.next_chunk() {
        match port.write(chunk) {
            Ok(WriteOutcome::Wrote(n)) if n > 0 => queue.advance(n),
            Ok(WriteOutcome::Closed) => {
                queue.clear();
                return;
            }
            // Would block, wrote nothing, or failed: the I/O thread retries or reaps.
            _ => return,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drains a queue through `CHUNK`-sized writes that the terminal takes whole.
    fn drain(queue: &mut Queue) -> Vec<u8> {
        let mut out = Vec::new();
        while let Some(chunk) = queue.next_chunk() {
            let n = chunk.len();
            out.extend_from_slice(chunk);
            queue.advance(n);
        }
        out
    }

    #[test]
    fn a_five_mebibyte_paste_is_queued_whole_and_leaves_in_order() {
        let paste: Vec<u8> = (0..5 * 1024 * 1024)
            .map(|i| b'a' + (i % 26) as u8)
            .collect();
        let mut queue = Queue::default();
        queue.push_input(b"echo ");
        queue.push_input(&paste);
        queue.push_input(b"\r");
        let mut expected = b"echo ".to_vec();
        expected.extend_from_slice(&paste);
        expected.push(b'\r');
        assert_eq!(
            drain(&mut queue),
            expected,
            "nothing dropped, nothing reordered"
        );
        assert!(queue.is_empty());
    }

    #[test]
    fn partial_writes_resume_where_they_stopped() {
        let mut queue = Queue::default();
        queue.push_input(b"abcdef");
        assert_eq!(queue.next_chunk(), Some(&b"abcdef"[..]));
        queue.advance(2);
        assert_eq!(queue.next_chunk(), Some(&b"cdef"[..]));
    }

    #[test]
    fn replies_beyond_the_cap_are_dropped_and_counted_but_input_is_not() {
        let mut queue = Queue::default();
        let reply = vec![b'r'; REPLY_CAP];
        assert_eq!(queue.push_reply(&reply), 0);
        assert_eq!(
            queue.push_reply(b"more"),
            4,
            "a full reply queue drops the reply"
        );
        queue.push_input(b"typed");
        let out = drain(&mut queue);
        assert_eq!(
            out.len(),
            REPLY_CAP + 5,
            "the input queued after the full reply is kept"
        );
    }

    #[test]
    fn reply_room_comes_back_once_the_reply_is_written() {
        let mut queue = Queue::default();
        queue.push_reply(&vec![b'r'; REPLY_CAP]);
        drain(&mut queue);
        assert_eq!(queue.push_reply(b"again"), 0);
    }
}
