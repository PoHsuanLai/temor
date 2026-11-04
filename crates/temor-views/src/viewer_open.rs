//! Opening a viewer from a terminal pane (PLAN §10): after a ⌘-click or the block toolbar's View,
//! the session answers the file it found, and the pane's link opens it (see `PaneLink::carry_out`).

use temor_client::model::vocab::ids::BlockId;

use super::pane_link::{PaneLink, Redraw};

/// The block toolbar's View: the files the block's output lists open beside the pane.
pub fn bar_view(link: PaneLink) -> impl FnMut(BlockId) + 'static {
    move |id: BlockId| {
        link.send(Redraw::WindowOnly, |s| s.view_block(id));
    }
}
