//! What the helper views ask of the window: each call runs on the hub, shows the messages it left,
//! and draws the window again.

use temor_core::remote::Grant;
use temor_core::vocab::ids::PaneId;

use super::desk::Desk;
use super::model::Hub;

impl Desk {
    fn with_remote(&self, run: impl FnOnce(&mut Hub)) {
        let notes = {
            let mut hub = self.hub.0.borrow_mut();
            run(&mut hub);
            std::mem::take(&mut hub.notices)
        };
        for note in notes {
            self.say(note);
        }
        self.bump();
    }

    /// The person answered whether the helper may be put on the host.
    pub fn remote_answer(&self, pane: PaneId, grant: Grant) {
        self.with_remote(|hub| hub.remote_answer(pane, grant));
    }

    /// The person asked to connect again.
    pub fn remote_retry(&self, pane: PaneId) {
        self.with_remote(|hub| hub.remote_retry(pane));
    }
}
