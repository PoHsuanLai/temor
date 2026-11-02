//! The terminal region's key wiring (PLAN §6): every key the region hears goes to the peek, the
//! input method and the session, and a chord the program does not take is the window's.

use std::rc::Rc;

use dioxus::prelude::{Key, Modifiers};
use ds::edit::composition::Composing;
use ds::edit::input::EditInput;
use ds::edit::raw_key::{KeyPhase, RawKey};
use ds::focus::request::FocusRequest;
use ds::host::measure::RectProbe;
use temor_client::model::vocab::ids::PaneId;

use super::ime::ImeGlue;
use super::pane_link::{PaneLink, Redraw};
use super::window_help::place_cursor;
use temor_pane_view::error::ViewError;
use temor_pane_view::input::stroke;
use temor_pane_view::session::{HistoryStep, Session};

/// A key the region hears, or the composer's field hands on, while the list shows the
/// composer: a typed key is the composer's (its text, Enter, Backspace, the arrows through the
/// history), and the caret goes to its field for the rest, but for Enter. A chord runs what the
/// keymap binds it to. Whether the composer took it: then the shell hears nothing.
fn to_composer(
    session: &mut Session,
    raw: &RawKey,
    focus: FocusRequest,
) -> Option<Result<(), ViewError>> {
    if !session.composer_shown() {
        return None;
    }
    if raw.phase == KeyPhase::Release {
        return Some(Ok(()));
    }
    if raw
        .modifiers
        .intersects(Modifiers::SUPER | Modifiers::META | Modifiers::CONTROL)
    {
        return Some(match stroke(raw, false) {
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
    link: PaneLink,
    ime: ImeGlue,
    apply: impl Fn(EditInput) + Clone + 'static,
    peek_key: Rc<dyn Fn(&RawKey)>,
    region: KeyRegion,
) -> impl FnMut(RawKey) + 'static {
    let KeyRegion { rect, composer, .. } = region;
    move |raw: RawKey| {
        peek_key(&raw);
        let (to_composer, mut left) = link.said(|s| to_composer(s, &raw, composer));
        let outcome = match to_composer.flatten() {
            Some(outcome) => Some(outcome),
            None => {
                let composing = match ime.composing.get() {
                    Composing::Active => true,
                    Composing::Idle | Composing::Cleared => false,
                };
                if raw.phase != KeyPhase::Release {
                    ime.settle(&apply);
                }
                if composing {
                    return;
                }
                // An unidentified key is ignored; the rest go to the session.
                let (outcome, keyed) =
                    link.said(|s| stroke(&raw, composing).map(|stroke| s.key(stroke)));
                left = keyed;
                outcome.flatten()
            }
        };
        if let Some(Err(err)) = outcome {
            link.desk.say(err.to_string());
        }
        // A chord that is not the program's is the window's or the workspace's, which the
        // desk sorts out.
        if link.carry_out(left) {
            return;
        }
        link.redraw(Redraw::WithGrid);
        place_cursor(ime.clone(), Rc::clone(&link.slot), rect);
    }
}
