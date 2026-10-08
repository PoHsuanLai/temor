//! Where events go.

use std::sync::mpsc::{Sender, SyncSender};

use super::VtEvent;

/// Receives [`VtEvent`]s. Called from inside `write`, on the writing thread, so it must not
/// block and must be `Send` (the terminal moves between threads with it).
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

impl EventSink for SyncSender<VtEvent> {
    /// Never blocks: when the channel is full the event is dropped, so size the channel for the
    /// burst a single write can produce.
    fn send(&self, event: VtEvent) {
        let _ = self.try_send(event);
    }
}
