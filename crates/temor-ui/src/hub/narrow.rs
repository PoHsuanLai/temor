//! A split is refused when it would leave a pane under the least width (PLAN §6): the workspace
//! step decides, from the width the window tells it here.

use temor_core::split::Columns;
use temor_core::workspace::WorkspaceIn;

use super::model::Hub;

impl Hub {
    /// `input`, with the focused pane's width in columns told to a split. A split of a pane that
    /// has not been laid out yet (no width known) goes ahead as asked.
    pub(super) fn measured(&self, input: WorkspaceIn) -> WorkspaceIn {
        let WorkspaceIn::Split { towards } = input else {
            return input;
        };
        match self.front_columns() {
            Some(width) => WorkspaceIn::SplitIn { towards, width },
            None => input,
        }
    }

    /// The width in columns of the tab in front's focused pane, once it has been laid out.
    fn front_columns(&self) -> Option<Columns> {
        let pane = self.ws.front().and_then(|tab| self.ws.focus_of(tab))?;
        let slot = self.slots.get(&pane)?;
        let fit = slot.borrow().session.as_ref()?.fit()?;
        Some(Columns(u32::from(fit.grid.cols.0)))
    }
}
