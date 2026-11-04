//! What the views ask of the window about agents: ⌘⇧U, and where the keyboard is. Each runs the
//! request and shows the notices it left; the hub draws the window again.

use crate::Desk;

impl Desk {
    /// ⌘⇧U: the next agent that waits for you takes the keyboard.
    pub fn jump_to_waiting(&self) {
        self.settle(crate::Hub::jump_to_waiting);
    }

    /// The pane has the keyboard on the page.
    pub fn pane_focused(&self, pane: temor_client::model::vocab::ids::PaneId) {
        self.hub.edit(|hub| hub.pane_focused(pane));
    }

    /// The keyboard left the pane.
    pub fn pane_blurred(&self, pane: temor_client::model::vocab::ids::PaneId) {
        // No redraw: a redraw tells the pane to take the keyboard back (see `pane_view`).
        self.hub.0.borrow_mut().pane_blurred(pane);
    }
}
