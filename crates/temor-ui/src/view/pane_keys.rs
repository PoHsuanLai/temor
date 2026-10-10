//! The terminal region's key wiring (PLAN §6): every key the region hears goes to the peek, the
//! input method and the session, and a chord the program does not take is the window's.

use std::cell::RefCell;
use std::rc::Rc;

use ds::edit::composition::Composing;
use ds::edit::input::EditInput;
use ds::edit::raw_key::{KeyPhase, RawKey};
use ds::host::measure::RectProbe;
use temor_core::vocab::ids::PaneId;
use temor_pane::keys::Action;
use temor_vt::Ime;

use super::ime::ImeGlue;
use super::slot::{Slot, repaint};
use super::window_help::{place_cursor, take_notice};
use crate::hub::{Desk, WindowAction, workspace_input};
use crate::input::stroke;

/// Every key the region hears (a press, a repeat, a release) as quire's raw key. `peek_key` is the
/// peek's ear for it; `apply` takes what a finished composition types.
pub fn key_handler(
    slot: Rc<RefCell<Slot>>,
    ime: ImeGlue,
    apply: impl Fn(EditInput) + Clone + 'static,
    desk: Desk,
    peek_key: Rc<dyn Fn(&RawKey)>,
    pane: PaneId,
    rect: RectProbe,
) -> impl FnMut(RawKey) + 'static {
    move |raw: RawKey| {
        peek_key(&raw);
        let composing = match ime.composing.get() {
            Composing::Active => Ime::Composing,
            Composing::Idle | Composing::Cleared => Ime::Idle,
        };
        if raw.phase != KeyPhase::Release {
            ime.settle(&apply);
        }
        if composing == Ime::Composing {
            return;
        }
        // An unidentified key is ignored; the rest go to the session.
        let outcome = match stroke(&raw, composing) {
            Some(stroke) => slot.borrow_mut().session.as_mut().map(|s| s.key(stroke)),
            None => None,
        };
        if let Some(Err(err)) = outcome {
            desk.say(err.to_string());
        }
        if let Some(message) = take_notice(&slot) {
            desk.say(message);
        }
        // A chord that is not the program's is the window's: the bar and the quick terminal
        // are its own; a tab or split chord is the workspace's. The quick terminal has no
        // tabs or splits of its own.
        let asked = slot
            .borrow_mut()
            .session
            .as_mut()
            .and_then(|s| s.take_asked());
        if let Some(Action::SelectSpace(number)) = asked {
            desk.select_space(number);
            return;
        }
        if asked == Some(Action::JumpToWaiting) {
            desk.jump_to_waiting();
            return;
        }
        if let Some(action) = asked {
            if desk.window_action(action) == WindowAction::Taken {
                return;
            }
            if desk.hub.is_quick(pane) {
                return;
            }
            if let Some(input) = workspace_input(action) {
                desk.act(input);
                return;
            }
        }
        desk.bump();
        repaint(&slot);
        place_cursor(ime.clone(), Rc::clone(&slot), rect);
    }
}
