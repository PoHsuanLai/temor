//! A window action picked in the command bar (PLAN §7): the same thing its chord does, run by
//! the hub. The bar has closed by now, so the keyboard pane is the pane the action is for.

use temor_pane::keys::Action;

use super::keys::workspace_input;
use super::model::Hub;

impl Hub {
    /// Does what `action`'s chord does.
    pub(super) fn run_window_action(&mut self, action: Action) {
        match action {
            Action::ToggleQuick => self.quick_toggle(),
            Action::JumpToWaiting => self.jump_to_waiting(),
            // The pane's own: the find bar and the jumps between prompts.
            Action::Find
            | Action::PreviousPrompt
            | Action::NextPrompt
            | Action::Copy
            | Action::Paste => self.run_in_keyboard_pane(action),
            // Already closing; the bar does not open itself.
            Action::OpenBar => {}
            other => {
                if let Some(input) = workspace_input(other) {
                    self.act(input);
                }
            }
        }
    }

    fn run_in_keyboard_pane(&mut self, action: Action) {
        let Some(slot) = self.keyboard_pane().and_then(|pane| self.slots.get(&pane)) else {
            return;
        };
        if let Some(session) = slot.borrow_mut().session.as_mut() {
            // A pane that cannot do it right now (a failed read) leaves things as they were.
            let _ = session.run(action);
        }
    }
}
