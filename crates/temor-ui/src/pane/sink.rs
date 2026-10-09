//! The window's event channel as a pane sink. The window drains it on its own thread.

use temor_pane::{EventSink, PaneEvent};
use tokio::sync::mpsc::UnboundedSender;

/// Sends a pane's events to the window's unbounded channel. A newtype, so the impl sits in this
/// crate (the orphan rule).
#[derive(Debug, Clone)]
pub struct UiSink(pub UnboundedSender<PaneEvent>);

impl EventSink for UiSink {
    fn send(&self, event: PaneEvent) {
        // A closed channel means the window is gone; the event has nowhere to go.
        let _ = self.0.send(event);
    }
}
