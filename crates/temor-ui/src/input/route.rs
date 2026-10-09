//! Where a key goes (PLAN §6): a chord the keymap binds runs its action; every other key is
//! encoded for the program, unless the input method is composing, which owns the keys until it
//! is done. Pure: no window, no terminal.

use temor_core::terminal::action::Action;
use temor_core::terminal::chord::Chord;
use temor_core::terminal::keymap::Keymap;
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
        (None, _) => Routed::Shell(stroke.key),
    }
}

#[cfg(test)]
mod tests {
    use temor_core::terminal::chord::{Chord, Mods};
    use temor_vt::{KeyCode, Modifiers};

    use super::*;

    fn press(code: KeyCode, text: &str, chord: Option<Chord>) -> Stroke {
        Stroke {
            chord,
            key: KeyInput::press(code, Modifiers::NONE, text),
        }
    }

    fn ctrl_shift(c: char) -> Chord {
        Chord::of_char(c, Mods::CTRL.with(Mods::SHIFT))
    }

    #[test]
    fn copy_and_paste_are_actions_and_never_reach_the_program() {
        let keymap = Keymap::defaults();
        let copy = press(KeyCode::C, "C", Some(ctrl_shift('c')));
        assert_eq!(
            route(copy, &keymap, FindBar::Closed),
            Routed::Action(Action::Copy)
        );
        let paste = press(KeyCode::V, "V", Some(ctrl_shift('v')));
        assert_eq!(
            route(paste, &keymap, FindBar::Open),
            Routed::Action(Action::Paste)
        );
    }

    #[test]
    fn ctrl_c_is_the_programs_interrupt() {
        let keymap = Keymap::defaults();
        let interrupt = press(KeyCode::C, "", Some(Chord::of_char('c', Mods::CTRL)));
        assert!(matches!(
            route(interrupt, &keymap, FindBar::Closed),
            Routed::Shell(_)
        ));
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
    fn plain_keys_go_to_the_program() {
        let keymap = Keymap::defaults();
        let key = press(KeyCode::A, "a", None);
        assert!(
            matches!(route(key.clone(), &keymap, FindBar::Closed), Routed::Shell(k) if k == key.key)
        );
    }
}
