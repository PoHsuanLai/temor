//! The tab in front and pinning (PLAN §7): what Pin Tab and Unpin Tab ask for. Running an action
//! picked in the bar is `dispatch`, as for every other route.

use temor_core::space::Pin;
use temor_core::vocab::ids::TabId;
use temor_core::vocab::text::Relaunch;
use temor_core::workspace::{TabsIn, WorkspaceIn};
use temor_pane::keys::Action;

use super::model::Hub;

/// The request that pins `tab`. A pinned tab opens again as a shell: it runs nothing of its own.
pub fn pin_as_shell(tab: TabId) -> TabsIn {
    TabsIn::Pin {
        tab,
        relaunch: Relaunch(String::new()),
    }
}

impl Hub {
    /// What Pin Tab or Unpin Tab asks of the tab in front; `None` for any other action, or with
    /// no tab.
    pub(super) fn pin_input(&self, action: Action) -> Option<WorkspaceIn> {
        let tab = self.ws.front()?;
        let tabs = match action {
            Action::PinTab => pin_as_shell(tab),
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
}
