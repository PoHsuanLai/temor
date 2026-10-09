//! Where a key goes (PLAN §6): a chord the keymap binds runs its action; every other key is
//! encoded for the program, unless the input method is composing, which owns the keys until it
//! is done. Pure: no window, no terminal.

use temor_pane::keys::Action;
use temor_pane::keys::Chord;
use temor_pane::keys::Keymap;
use temor_vt::{Ime, KeyAction, KeyInput};

/// A key as the window heard it: its chord (when it has one) and what the program would get.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stroke {
    pub chord: Option<Chord>,
    pub key: KeyInput,
}

/// Whether the find bar is open: its keys (Escape, next, previous) belong to it only then.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FindBar {
    Open,
    Closed,
}

/// Where a key goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Routed {
    /// The pane runs this action; the program never sees the key.
    Action(Action),
    /// The program gets the key, encoded by the terminal's modes.
    Shell(KeyInput),
    /// Nothing: the input method is composing, or the key means nothing here.
    Ignored,
}

/// Routes one key event (PLAN §4: a machine never takes a key the shell would get).
pub fn route(stroke: Stroke, keymap: &Keymap, find: FindBar) -> Routed {
    if stroke.key.ime == Ime::Composing {
        return Routed::Ignored;
    }
    if stroke.key.action == KeyAction::Release {
        return Routed::Shell(stroke.key);
    }
    let bound = stroke.chord.and_then(|chord| keymap.action_for(&chord));
    match (bound, find) {
        // Escape means the shell's when no find bar is open; the find keys do nothing there.
        (Some(Action::CloseFind), FindBar::Closed) => Routed::Shell(stroke.key),
        (Some(Action::FindNext | Action::FindPrevious), FindBar::Closed) => Routed::Ignored,
        (Some(action), _) => Routed::Action(action),
        // An unbound ⌘ letter is dropped, never typed into the program (PLAN §6; Ghostty does the
        // same). Super with Ctrl is not a ⌘ chord and goes to the program as before.
        (None, _) if stroke.chord.as_ref().is_some_and(is_unbound_command_letter) => {
            Routed::Ignored
        }
        (None, _) => Routed::Shell(stroke.key),
    }
}

/// A ⌘ chord (Super, without Ctrl) on a letter that no binding uses. Super digits and
/// Super+Space are the shell's (PLAN §6), so they keep their old route.
fn is_unbound_command_letter(chord: &Chord) -> bool {
    chord.is_command_form()
        && matches!(chord.key, temor_pane::keys::ChordKey::Char(c) if c.is_alphabetic())
}

#[cfg(test)]
mod tests {
    use temor_pane::keys::Chord;
    use temor_vt::{KeyCode, Modifiers};

    use super::*;

    fn press(code: KeyCode, text: &str, chord: Option<Chord>) -> Stroke {
        Stroke {
            chord,
            key: KeyInput::press(code, Modifiers::NONE, text),
        }
    }

    #[test]
    fn escape_is_the_shell_s_unless_the_find_bar_is_open() {
        let keymap = Keymap::defaults();
        let escape = || Stroke {
            chord: Some(Chord::parse("escape").expect("valid")),
            key: KeyInput::press(KeyCode::Escape, Modifiers::NONE, ""),
        };
        assert!(matches!(
            route(escape(), &keymap, FindBar::Closed),
            Routed::Shell(_)
        ));
        assert_eq!(
            route(escape(), &keymap, FindBar::Open),
            Routed::Action(Action::CloseFind)
        );
    }

    #[test]
    fn composing_keys_belong_to_the_input_method() {
        let keymap = Keymap::defaults();
        let mut stroke = press(KeyCode::A, "a", None);
        stroke.key.ime = Ime::Composing;
        assert_eq!(route(stroke, &keymap, FindBar::Closed), Routed::Ignored);
    }

    #[test]
    fn an_unbound_command_letter_is_dropped_and_control_chords_still_reach_the_program() {
        let keymap = Keymap::defaults();
        #[rustfmt::skip]
        let rows: &[(&str, bool)] = &[
            ("super+z", true),
            ("super+5", false),
            ("super+space", false),
            ("ctrl+z", false),
            ("ctrl+shift+z", false),
        ];
        for (text, dropped) in rows {
            let chord = Chord::parse(text).expect("valid");
            let stroke = Stroke {
                chord: Some(chord),
                key: KeyInput::press(KeyCode::A, Modifiers::NONE, "z"),
            };
            let got = route(stroke.clone(), &keymap, FindBar::Closed);
            if *dropped {
                assert_eq!(got, Routed::Ignored, "{text}");
            } else {
                assert!(matches!(got, Routed::Shell(_)), "{text}: {got:?}");
            }
        }
    }

    #[test]
    fn plain_keys_go_to_the_program() {
        let keymap = Keymap::defaults();
        let key = press(KeyCode::A, "a", None);
        assert!(
            matches!(route(key.clone(), &keymap, FindBar::Closed), Routed::Shell(k) if k == key.key)
        );
    }
}
