//! A window action picked in the command bar (PLAN §7): the same thing its chord does, run by
//! the hub. The bar has closed by now, so the keyboard pane is the pane the action is for.

use temor_core::space::Pin;
use temor_core::vocab::text::Relaunch;
use temor_core::workspace::{TabsIn, WorkspaceIn};
use temor_pane::keys::Action;

use super::keys::workspace_input;
use super::model::Hub;

impl Hub {
    /// Does what `action`'s chord does.
    pub(super) fn run_window_action(&mut self, action: Action) {
        match action {
            Action::ToggleQuick => self.quick_toggle(),
            Action::JumpToWaiting => self.jump_to_waiting(),
            Action::ToggleSidebar
            | Action::ShowDownloads
            | Action::ZoomIn
            | Action::ZoomOut
            | Action::ActualSize => self.window_asks.push(action),
            Action::PinTab | Action::UnpinTab => {
                if let Some(input) = self.pin_input(action) {
                    self.act(input);
                }
            }
            // The pane's own: the find bar and picking, copying and bookmarking commands.
            Action::Find
            | Action::PreviousPrompt
            | Action::NextPrompt
            | Action::CopyCommands
            | Action::CopyOutputs
            | Action::SelectAllBlocks
            | Action::ToggleBookmark
            | Action::PreviousBookmark
            | Action::NextBookmark
            | Action::ReinputCommands
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

    /// What Pin Tab or Unpin Tab asks of the tab in front; `None` for any other action, or with
    /// no tab. A pinned tab opens again as a shell: it runs nothing of its own.
    pub(super) fn pin_input(&self, action: Action) -> Option<WorkspaceIn> {
        let tab = self.ws.front()?;
        let tabs = match action {
            Action::PinTab => TabsIn::Pin {
                tab,
                relaunch: Relaunch(String::new()),
            },
            Action::UnpinTab => TabsIn::Unpin(tab),
            _ => return None,
        };
        Some(WorkspaceIn::Tabs(tabs))
    }

    /// Whether the tab in front is pinned; `None` with no tab.
    pub(super) fn front_pinned(&self) -> Option<bool> {
        let tab = self.ws.tabs.tab(self.ws.front()?)?;
        Some(matches!(tab.pin, Pin::Pinned { .. }))
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
