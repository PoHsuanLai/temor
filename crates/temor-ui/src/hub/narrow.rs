//! A split is refused when it would leave a pane under the least width (PLAN §6): the workspace
//! step decides, from the width the window tells it here.

use dioxus::prelude::WritableExt;
use temor_client::model::split::Columns;
use temor_client::model::vocab::ids::PaneId;
use temor_client::model::workspace::WorkspaceIn;

use super::desk::Desk;
use super::model::Hub;
use crate::view::sidebar_room::toggled;

impl Hub {
    /// `input`, with the focused pane's width in columns told to a split, and the width of the
    /// pane a tab is dropped on told to the drop. One whose pane has not been laid out yet (no
    /// width known) goes ahead as asked.
    pub(super) fn measured(&self, input: WorkspaceIn) -> WorkspaceIn {
        match input {
            WorkspaceIn::Split { towards } => match self.front_columns() {
                Some(width) => WorkspaceIn::SplitIn { towards, width },
                None => input,
            },
            // A tab dropped beside a pane splits it as a split does.
            WorkspaceIn::DropTab {
                from,
                onto,
                towards,
            } => match self.columns_of(onto) {
                Some(width) => WorkspaceIn::DropTabIn {
                    from,
                    onto,
                    towards,
                    width,
                },
                None => input,
            },
            other => other,
        }
    }

    /// The width in columns of the tab in front's focused pane, once it has been laid out.
    fn front_columns(&self) -> Option<Columns> {
        let pane = self
            .book
            .ws
            .front()
            .and_then(|tab| self.book.ws.focus_of(tab))?;
        self.columns_of(pane)
    }

    /// The width in columns of `pane`, once it has been laid out.
    fn columns_of(&self, pane: PaneId) -> Option<Columns> {
        let slot = self.slots.get(&pane)?;
        let fit = slot.borrow().session.as_ref()?.fit()?;
        Some(Columns(u32::from(fit.grid.cols.0)))
    }
}

impl Desk {
    /// Shows the sidebar, or hides it, whatever the window's width.
    pub(super) fn sidebar_toggle(&self) {
        let mut sidebar = self.sidebar;
        let next = toggled(sidebar());
        sidebar.set(next);
    }
}
