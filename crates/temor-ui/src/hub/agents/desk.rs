//! What the views ask of the window about agents: ⌘⇧U, a pick in a tab's menu, the sheet's
//! buttons. Each runs the request, shows the notices it left and draws the window again.

use temor_core::vocab::ids::TabId;

use super::menu::AgentPick;
use crate::hub::Desk;

impl Desk {
    /// ⌘⇧U: the next agent that waits for you takes the keyboard.
    pub fn jump_to_waiting(&self) {
        self.settle(crate::hub::Hub::jump_to_waiting);
    }

    /// `tab`'s menu is opening: its run's restore points are read again.
    pub fn agent_menu_opened(&self, tab: TabId) {
        self.settle(|hub| hub.refresh_points_of(tab));
    }

    /// A choice in `tab`'s menu.
    pub fn agent_pick(&self, tab: TabId, pick: AgentPick) {
        self.settle(|hub| hub.agent_pick(tab, pick));
    }

    /// Restore…, in the sheet that showed what it would change.
    pub fn perform_restore(&self) {
        self.settle(crate::hub::Hub::perform_restore);
    }

    /// The sheet is closed.
    pub fn close_agent_sheet(&self) {
        self.settle(crate::hub::Hub::close_agent_sheet);
    }

    /// A leftover worktree opens in a tab.
    pub fn open_leftover(&self, path: String) {
        self.settle(|hub| hub.open_leftover(&path));
    }

    /// A leftover worktree and its branch are removed.
    pub fn discard_leftover(&self, path: String) {
        self.settle(|hub| hub.discard_leftover(&path));
    }

    /// The pane has the keyboard on the page.
    pub fn pane_focused(&self, pane: temor_core::vocab::ids::PaneId) {
        self.hub.0.borrow_mut().pane_focused(pane);
        self.bump();
    }

    /// The keyboard left the pane.
    pub fn pane_blurred(&self, pane: temor_core::vocab::ids::PaneId) {
        // No redraw: a redraw tells the pane to take the keyboard back (see `pane_view`).
        self.hub.0.borrow_mut().pane_blurred(pane);
    }
}
