//! Where events and replies go.

use std::sync::mpsc::Sender;

use super::{ClipboardAnswer, ClipboardRead, VtEvent};

/// Receives [`VtEvent`]s. Called from inside `write` (and `resize`), on the calling thread, so it
/// must not block and must be `Send` (the terminal moves between threads with it).
///
/// **Never lock the terminal from a sink**: the sink runs while the terminal's lock is held by
/// the writer, so locking it again deadlocks. A sink that panics loses that one event; the
/// terminal counts it (`Terminal::dropped_events`).
pub trait EventSink: Send {
    /// Takes one event.
    fn send(&self, event: VtEvent);
}

impl EventSink for Sender<VtEvent> {
    // A closed receiver means nobody listens any more; the event has nowhere to go.
    fn send(&self, event: VtEvent) {
        let _ = Sender::send(self, event);
    }
}

/// Receives the bytes the terminal answers the program with (device and mode reports, size
/// reports, XTVERSION, ...). These go straight back to the program's input (PLAN §4b) and must
/// never be dropped: a program waiting for a reply hangs without it. An implementation writes
/// them to the PTY or queues them without a bound. Same threading rules as [`EventSink`].
pub trait ReplySink: Send {
    /// Takes one reply, in the order the terminal produced it.
    fn reply(&self, bytes: &[u8]);
}

impl ReplySink for Sender<Vec<u8>> {
    // Unbounded, so nothing is lost while the receiver lives; a closed receiver means the
    // program's input is gone too.
    fn reply(&self, bytes: &[u8]) {
        let _ = self.send(bytes.to_vec());
    }
}

/// Answers a program's clipboard reads (OSC 52 with `?`). libghostty wants the answer before the
/// read returns, so it is given from what the host already holds, never by asking the window: an
/// implementation reads a copy the host keeps up to date and returns at once. Same threading
/// rules as [`EventSink`]. A terminal without a source refuses every read.
pub trait ClipboardSource: Send {
    /// The answer to `read`.
    fn read(&self, read: &ClipboardRead) -> ClipboardAnswer;
}
