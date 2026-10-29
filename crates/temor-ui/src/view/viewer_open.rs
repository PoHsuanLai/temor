//! Opening a viewer from a terminal pane (PLAN §10): after a ⌘-click or the block toolbar's View,
//! the session holds the file it found; this hands it to the window, which splits the pane.

use std::cell::RefCell;
use std::rc::Rc;

use temor_client::model::shelf::PaneSite;
use temor_client::model::vocab::ids::{BlockId, PaneId};

use super::slot::Slot;
use super::window_help::take_notice;
use crate::hub::Desk;
use crate::session::Session;

/// Opens the viewer the pane's session asked for, if it asked for one.
pub fn view_if_asked(slot: &RefCell<Slot>, desk: &Desk, pane: PaneId) {
    let ask = slot
        .borrow_mut()
        .session
        .as_mut()
        .and_then(Session::take_view_ask);
    if let Some(ask) = ask {
        // A file on another computer is fetched first; the viewer opens when it has arrived.
        let notes = match ask.site {
            PaneSite::Local => desk.hub.open_viewer(pane, ask),
            PaneSite::Remote => desk.hub.open_host_viewer(pane, &ask.file.to_string_lossy()),
        };
        for text in notes {
            desk.say(text);
        }
        desk.bump();
    }
}

/// The block toolbar's View: the files the block's output lists open beside the pane.
pub fn bar_view(
    slot: Rc<RefCell<Slot>>,
    desk: Desk,
    pane: PaneId,
) -> impl FnMut(BlockId) + 'static {
    move |id: BlockId| {
        if let Some(session) = slot.borrow_mut().session.as_mut() {
            session.view_block(id);
        }
        if let Some(text) = take_notice(&slot) {
            desk.say(text);
        }
        view_if_asked(&slot, &desk, pane);
        desk.bump();
    }
}
