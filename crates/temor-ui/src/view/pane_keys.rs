//! The terminal region's key wiring (PLAN §6): every key the region hears goes to the peek, the
//! input method and the session, and a chord the program does not take is the window's.

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::{Key, Modifiers};
use ds::edit::composition::Composing;
use ds::edit::input::EditInput;
use ds::edit::raw_key::{KeyPhase, RawKey};
use ds::focus::request::FocusRequest;
use ds::host::measure::RectProbe;
use temor_client::model::vocab::ids::PaneId;
use temor_panes::vt::Ime;

use super::ime::ImeGlue;
use super::slot::{Slot, repaint};
use super::window_help::{place_cursor, take_notice};
use crate::hub::{Desk, Origin};
use crate::input::stroke;
use crate::session::HistoryStep;
use temor_pane_view::error::ViewError;

/// A key the region hears, or the composer's field hands on, while the list shows the
/// composer: a typed key is the composer's (its text, Enter, Backspace, the arrows through the
/// history), and the caret goes to its field for the rest, but for Enter. A chord runs what the
/// keymap binds it to. Whether the composer took it: then the shell hears nothing.
fn to_composer(
    slot: &RefCell<Slot>,
    raw: &RawKey,
    focus: FocusRequest,
) -> Option<Result<(), ViewError>> {
    let mut slot = slot.borrow_mut();
    let session = slot.session.as_mut().filter(|s| s.composer_shown())?;
    if raw.phase == KeyPhase::Release {
        return Some(Ok(()));
    }
    if raw
        .modifiers
        .intersects(Modifiers::SUPER | Modifiers::META | Modifiers::CONTROL)
    {
        return Some(match stroke(raw, Ime::Idle) {
            Some(stroke) => session.composer_chord(stroke),
            None => Ok(()),
        });
    }
    match (&raw.key, &raw.text) {
        // The command runs and the composer goes: the keys stay the terminal's.
        (Key::Enter, _) => {
            session.composer_submit();
            return Some(Ok(()));
        }
        (Key::Backspace, _) => session.composer_backspace(),
        (Key::ArrowUp, _) => session.composer_history(HistoryStep::Older),
        (Key::ArrowDown, _) => session.composer_history(HistoryStep::Newer),
        (_, Some(text)) => session.composer_append(text),
        _ => {}
    }
    focus.request();
    Some(Ok(()))
}

/// Where the region's keys go: its pane, its box (the input method is placed in it), and the
/// composer's field, which takes the typing while the list shows it.
#[derive(Clone, Copy)]
pub struct KeyRegion {
    pub pane: PaneId,
    pub rect: RectProbe,
    pub composer: FocusRequest,
}

impl std::fmt::Debug for KeyRegion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyRegion")
            .field("pane", &self.pane)
            .finish_non_exhaustive()
    }
}

/// Every key the region hears (a press, a repeat, a release) as quire's raw key. `peek_key` is the
/// peek's ear for it; `apply` takes what a finished composition types.
pub fn key_handler(
    slot: Rc<RefCell<Slot>>,
    ime: ImeGlue,
    apply: impl Fn(EditInput) + Clone + 'static,
    desk: Desk,
    peek_key: Rc<dyn Fn(&RawKey)>,
    region: KeyRegion,
) -> impl FnMut(RawKey) + 'static {
    let KeyRegion {
        pane,
        rect,
        composer,
    } = region;
    move |raw: RawKey| {
        peek_key(&raw);
        let outcome = match to_composer(&slot, &raw, composer) {
            Some(outcome) => Some(outcome),
            None => {
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
                match stroke(&raw, composing) {
                    Some(stroke) => slot.borrow_mut().session.as_mut().map(|s| s.key(stroke)),
                    None => None,
                }
            }
        };
        if let Some(Err(err)) = outcome {
            desk.say(err.to_string());
        }
        if let Some(message) = take_notice(&slot) {
            desk.say(message);
        }
        // A chord that is not the program's is the window's or the workspace's, which the
        // desk sorts out; the quick terminal has no tabs or splits of its own.
        let asked = slot
            .borrow_mut()
            .session
            .as_mut()
            .and_then(|s| s.take_asked());
        if let Some(action) = asked {
            let from = if desk.hub.is_quick(pane) {
                Origin::Quick
            } else {
                Origin::Terminal
            };
            desk.run_action(action, from);
            return;
        }
        desk.bump();
        repaint(&slot);
        place_cursor(ime.clone(), Rc::clone(&slot), rect);
    }
}
