//! What the helper views ask of the window: each call runs on the hub, shows the messages it left,
//! and draws the window again.

use temor_core::remote::Grant;
use temor_core::vocab::ids::PaneId;

use super::desk::Desk;

impl Desk {
    /// The person answered whether the helper may be put on the host.
    pub fn remote_answer(&self, pane: PaneId, grant: Grant) {
        self.settle(|hub| hub.remote_answer(pane, grant));
    }

    /// The person asked to connect again.
    pub fn remote_retry(&self, pane: PaneId) {
        self.settle(|hub| hub.remote_retry(pane));
    }
}
