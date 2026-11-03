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

/// What a key does while the list shows the composer: a typed key is the composer's (its text,
/// Enter, Backspace, the arrows through the history), and a chord runs what the keymap binds it
/// to.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ComposerKey {
    /// A key coming up: nothing.
    Release,
    /// A chord (Super, Meta or Ctrl held): the keymap's.
    Chord,
    /// Enter: the command runs and the composer goes; the keys stay the terminal's.
    Submit,
    Backspace,
    History(HistoryStep),
    Type(String),
    /// Anything else: nothing but the caret going to the field.
    Other,
}

fn composer_key(raw: &RawKey) -> ComposerKey {
    let chord = Modifiers::SUPER | Modifiers::META | Modifiers::CONTROL;
    match (raw.phase, &raw.key, &raw.text) {
        (KeyPhase::Release, ..) => ComposerKey::Release,
        _ if raw.modifiers.intersects(chord) => ComposerKey::Chord,
        (_, Key::Enter, _) => ComposerKey::Submit,
        (_, Key::Backspace, _) => ComposerKey::Backspace,
        (_, Key::ArrowUp, _) => ComposerKey::History(HistoryStep::Older),
        (_, Key::ArrowDown, _) => ComposerKey::History(HistoryStep::Newer),
        (_, _, Some(text)) => ComposerKey::Type(text.clone()),
        _ => ComposerKey::Other,
    }
}

/// A key the region hears, or the composer's field hands on, while the list shows the composer
/// (see [`composer_key`]). Typing puts the caret in the composer's field. Whether the composer
/// took it: then the shell hears nothing.
fn to_composer(
    session: &mut Session,
    raw: &RawKey,
    focus: FocusRequest,
) -> Option<Result<(), ViewError>> {
    if !session.composer_shown() {
        return None;
    }
    let key = composer_key(raw);
    let typing = matches!(
        key,
        ComposerKey::Backspace
            | ComposerKey::History(_)
            | ComposerKey::Type(_)
            | ComposerKey::Other
    );
    match key {
        ComposerKey::Chord => {
            return Some(
                stroke(raw, false).map_or(Ok(()), |stroke| session.composer_chord(stroke)),
            );
        }
        ComposerKey::Submit => session.composer_submit(),
        ComposerKey::Backspace => session.composer_backspace(),
        ComposerKey::History(step) => session.composer_history(step),
        ComposerKey::Type(text) => session.composer_append(&text),
        ComposerKey::Release | ComposerKey::Other => {}
    }
    if typing {
        focus.request();
    }
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

#[cfg(test)]
mod tests {
    use dioxus::html::{Code, Location};
    use dioxus::prelude::{Key, Modifiers};
    use ds::edit::raw_key::{KeyPhase, RawKey};
    use temor_pane_view::session::HistoryStep;

    use super::{ComposerKey, composer_key};

    fn raw(phase: KeyPhase, key: Key, modifiers: Modifiers, text: Option<&str>) -> RawKey {
        RawKey {
            phase,
            key,
            code: Code::Unidentified,
            modifiers,
            location: Location::Standard,
            text: text.map(str::to_owned),
        }
    }

    #[test]
    fn the_composer_takes_typing_and_leaves_chords_to_the_keymap() {
        use KeyPhase::{Press, Release, Repeat};
        let none = Modifiers::empty();
        let a = || Key::Character("a".to_owned());
        // (the key, what it does)
        let cases = [
            (
                raw(Press, a(), none, Some("a")),
                ComposerKey::Type("a".to_owned()),
            ),
            (
                raw(Repeat, a(), none, Some("a")),
                ComposerKey::Type("a".to_owned()),
            ),
            (raw(Release, a(), none, None), ComposerKey::Release),
            (raw(Release, Key::Enter, none, None), ComposerKey::Release),
            (
                raw(Press, a(), Modifiers::CONTROL, None),
                ComposerKey::Chord,
            ),
            (
                raw(Press, Key::Enter, Modifiers::SUPER, None),
                ComposerKey::Chord,
            ),
            (
                raw(Press, Key::Enter, none, Some("\r")),
                ComposerKey::Submit,
            ),
            (
                raw(Press, Key::Backspace, none, None),
                ComposerKey::Backspace,
            ),
            (
                raw(Press, Key::ArrowUp, none, None),
                ComposerKey::History(HistoryStep::Older),
            ),
            (
                raw(Press, Key::ArrowDown, Modifiers::SHIFT, None),
                ComposerKey::History(HistoryStep::Newer),
            ),
            (raw(Press, Key::Escape, none, None), ComposerKey::Other),
        ];
        for (key, want) in cases {
            assert_eq!(composer_key(&key), want, "{key:?}");
        }
    }
}
