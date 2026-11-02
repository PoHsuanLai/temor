//! The window's way to a pane's session (PLAN §4c, thin edges): a call runs through
//! [`Session::said`], and what it left (a message, a tab or split a key asked for, a file to
//! view) is carried out here, then the window is drawn again.

use std::cell::RefCell;
use std::rc::Rc;

use temor_client::model::shelf::PaneSite;
use temor_client::model::vocab::ids::PaneId;
use temor_hub::{Desk, Origin};
use temor_pane_view::session::{Left, Session};
use temor_ui_base::slot::{Slot, repaint};

/// What a call changes on screen. A copy changes nothing the grid shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Redraw {
    /// The window is drawn again; the grid is as it was.
    WindowOnly,
    /// The grid is painted again too.
    WithGrid,
}

/// One pane's session, as the window reaches it.
#[derive(Clone)]
pub struct PaneLink {
    pub slot: Rc<RefCell<Slot>>,
    pub desk: Desk,
    pub pane: PaneId,
}

impl std::fmt::Debug for PaneLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaneLink")
            .field("pane", &self.pane)
            .finish_non_exhaustive()
    }
}

impl PaneLink {
    /// Runs `call` on the session, if the pane has one, carries out what it left, and draws the
    /// window (with `redraw`, the grid too) again.
    pub fn send<R>(&self, redraw: Redraw, call: impl FnOnce(&mut Session) -> R) -> Option<R> {
        let (answer, left) = self.said(call);
        self.carry_out(left);
        self.redraw(redraw);
        answer
    }

    /// Runs `call` on the session, if the pane has one, and what it left, not yet carried out.
    pub fn said<R>(&self, call: impl FnOnce(&mut Session) -> R) -> (Option<R>, Left) {
        let said = self
            .slot
            .borrow_mut()
            .session
            .as_mut()
            .map(|session| session.said(call));
        match said {
            Some((answer, left)) => (Some(answer), left),
            None => (None, Left::default()),
        }
    }

    /// Says the message, opens the file and runs the action a call left. Whether an action ran:
    /// the workspace draws what it changes, and the keyboard may have left the pane.
    pub fn carry_out(&self, left: Left) -> bool {
        let Left {
            notice,
            asked,
            view,
        } = left;
        if let Some(text) = notice {
            self.desk.say(text);
        }
        if let Some(ask) = view {
            // A file on another computer is fetched first; the viewer opens when it has arrived.
            let notes = match ask.site {
                PaneSite::Local => self.desk.hub.open_viewer(self.pane, ask),
                PaneSite::Remote => self
                    .desk
                    .hub
                    .open_host_viewer(self.pane, &ask.file.to_string_lossy()),
            };
            for text in notes {
                self.desk.say(text);
            }
            self.desk.bump();
        }
        // The quick terminal has no tabs or splits of its own.
        let Some(action) = asked else {
            return false;
        };
        let from = match self.desk.hub.is_quick(self.pane) {
            true => Origin::Quick,
            false => Origin::Terminal,
        };
        self.desk.run_action(action, from);
        true
    }

    /// Carries out what the session's own handlers left: a ⌘-click's file once the press is
    /// released (opening it splits the pane, which must not happen while the press holds).
    pub fn settle(&self) {
        let (_, left) = self.said(|_| ());
        self.carry_out(left);
    }

    /// Draws the window again, and with [`Redraw::WithGrid`] the pane.
    pub fn redraw(&self, redraw: Redraw) {
        self.desk.bump();
        if redraw == Redraw::WithGrid {
            repaint(&self.slot);
        }
    }
}
