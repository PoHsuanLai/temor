//! Where a pane's events go. The engine never waits on its sink: a send queues or drops.

use std::sync::mpsc::Sender;

use super::event::PaneEvent;

/// Receives a pane's events. The I/O loop and the window's own calls both send, so `send` must
/// not block. A sink whose receiver is gone drops the event.
pub trait EventSink: Send + Sync {
    /// Delivers one event.
    fn send(&self, event: PaneEvent);
}

impl EventSink for Sender<PaneEvent> {
    fn send(&self, event: PaneEvent) {
        let _ = Sender::send(self, event);
    }
}
