//! Unit tests for click counting and the key stroke.

use std::time::Duration;

use temor_pane::text::CellPos;

use super::click::{Click, MULTI_CLICK, next_click};

#[test]
fn quick_presses_at_one_cell_count_up_then_wrap() {
    let cell = CellPos::new(3, 4);
    let (first, state) = next_click(None, Duration::from_secs(10), cell);
    assert_eq!(first, Click::Single);
    let (second, state) = next_click(Some(state), Duration::from_millis(10_200), cell);
    assert_eq!(second, Click::Double);
    let (third, state) = next_click(Some(state), Duration::from_millis(10_300), cell);
    assert_eq!(third, Click::Triple);
    let (fourth, _) = next_click(Some(state), Duration::from_millis(10_400), cell);
    assert_eq!(fourth, Click::Single);
}

#[test]
fn a_slow_press_or_another_cell_starts_again() {
    let cell = CellPos::new(0, 0);
    let (_, state) = next_click(None, Duration::ZERO, cell);
    let (slow, _) = next_click(Some(state), MULTI_CLICK + Duration::from_millis(1), cell);
    assert_eq!(slow, Click::Single);
    let (moved, _) = next_click(Some(state), Duration::from_millis(50), CellPos::new(0, 1));
    assert_eq!(moved, Click::Single);
}

#[test]
fn an_unidentified_key_is_not_sent_as_a_space() {
    use dioxus::html::{Code, Key, Location, Modifiers};
    use ds::edit::raw_key::{KeyPhase, RawKey};
    use temor_vt::Ime;

    use super::keys::stroke;
    let raw = RawKey {
        phase: KeyPhase::Press,
        key: Key::Unidentified,
        code: Code::Unidentified,
        modifiers: Modifiers::empty(),
        location: Location::Standard,
        text: None,
        unshifted: None,
    };
    assert!(stroke(&raw, Ime::Idle).is_none());
}

/// The window's key events as the desktop delivers them, through the stroke and the router: a
/// ⌘ chord (Ctrl+Shift or Super form) runs its action, and everything else reaches the program.
#[test]
fn keys_as_the_desktop_sends_them_reach_the_action_or_the_program() {
    use dioxus::html::{Code, Key, Location, Modifiers as Held};
    use ds::edit::raw_key::{KeyPhase, RawKey};
    use temor_pane::keys::{Action, Keymap};
    use temor_vt::Ime;

    use super::keys::stroke;
    use crate::input::{FindBar, Routed, route};

    let keymap = Keymap::defaults();
    let ctrl_shift = Held::CONTROL | Held::SHIFT;
    // (name, logical key, physical key, held modifiers, the action it runs, if any)
    let rows: &[(&str, Key, Code, Held, Option<Action>)] = &[
        (
            "⌘C as Ctrl+Shift+C",
            Key::Character("C".into()),
            Code::KeyC,
            ctrl_shift,
            Some(Action::Copy),
        ),
        (
            "⌘C as Super+C",
            Key::Character("c".into()),
            Code::KeyC,
            Held::SUPER,
            Some(Action::Copy),
        ),
        (
            "⌘C as Meta+C",
            Key::Character("c".into()),
            Code::KeyC,
            Held::META,
            Some(Action::Copy),
        ),
        (
            "⌘⌥G as Super+Alt+G",
            Key::Character("g".into()),
            Code::KeyG,
            Held::SUPER | Held::ALT,
            Some(Action::FindPrevious),
        ),
        (
            "Ctrl+C is the interrupt",
            Key::Character("c".into()),
            Code::KeyC,
            Held::CONTROL,
            None,
        ),
        ("Super alone", Key::Meta, Code::MetaLeft, Held::SUPER, None),
        (
            "Super+Space",
            Key::Character(" ".into()),
            Code::Space,
            Held::SUPER,
            None,
        ),
        ("Super+Tab", Key::Tab, Code::Tab, Held::SUPER, None),
        (
            "Ctrl+Space stays with the input method",
            Key::Character(" ".into()),
            Code::Space,
            Held::CONTROL,
            None,
        ),
    ];
    for (name, key, code, held, want) in rows {
        let raw = RawKey {
            phase: KeyPhase::Press,
            key: key.clone(),
            code: *code,
            modifiers: *held,
            location: Location::Standard,
            text: None,
            unshifted: None,
        };
        let stroke = stroke(&raw, Ime::Idle).unwrap_or_else(|| panic!("{name}: no stroke"));
        // The find bar is open, so the find chords run (with it closed they do nothing).
        let got = match route(stroke, &keymap, FindBar::Open) {
            Routed::Action(action) => Some(action),
            Routed::Shell(_) => None,
            Routed::Ignored => panic!("{name}: ignored"),
        };
        assert_eq!(got, *want, "{name}");
    }
}

/// Shifted keys as the desktop reports them: the logical key is the shifted character, and the
/// chord must still be the base key with Shift held. Every ⌘ chord is an action, never a control
/// byte for the program (Ctrl+Shift+W must not arrive as Ctrl+W).
#[test]
fn shifted_command_chords_match_with_or_without_the_unshifted_key() {
    use dioxus::html::{Code, Key, Location, Modifiers as Held};
    use ds::edit::raw_key::{KeyPhase, RawKey};
    use temor_pane::keys::{Action, Keymap};
    use temor_vt::Ime;

    use super::keys::stroke;
    use crate::input::{FindBar, Routed, route};

    let keymap = Keymap::defaults();
    let cs = Held::CONTROL | Held::SHIFT;
    // (name, logical key, physical key, modifiers, unshifted key if reported, the action)
    #[rustfmt::skip]
    #[allow(clippy::type_complexity)] // a table of named rows
    let rows: &[(&str, &str, Code, Held, Option<&str>, Option<Action>)] = &[
        ("⌘1 with no unshifted key", "!", Code::Digit1, cs, None, Some(Action::SelectTab(1))),
        ("⌘1 with the unshifted key", "!", Code::Digit1, cs, Some("1"), Some(Action::SelectTab(1))),
        ("⌘9 with no unshifted key", "(", Code::Digit9, cs, None, Some(Action::SelectTab(9))),
        ("⌃⌘= as Ctrl+Shift+=", "+", Code::Equal, cs, None, Some(Action::Equalize)),
        ("⌃⌘= as Ctrl+Super+=", "=", Code::Equal, Held::CONTROL | Held::SUPER, None, Some(Action::Equalize)),
        ("⌘W as Ctrl+Shift+W", "W", Code::KeyW, cs, None, Some(Action::ClosePane)),
        ("⌘T as Ctrl+Shift+T", "T", Code::KeyT, cs, None, Some(Action::NewTab)),
        ("⌘D as Ctrl+Shift+D", "D", Code::KeyD, cs, None, Some(Action::SplitRight)),
        ("⌘⇧D as Ctrl+Shift+Alt+D", "D", Code::KeyD, cs | Held::ALT, None, Some(Action::SplitDown)),
        ("⌘⇧D as Super+Shift+D", "D", Code::KeyD, Held::SUPER | Held::SHIFT, None, Some(Action::SplitDown)),
        ("unbound Ctrl+Shift+Z is dropped", "Z", Code::KeyZ, cs, None, None),
        ("unbound Super+Z is dropped", "z", Code::KeyZ, Held::SUPER, None, None),
    ];
    for (name, key, code, held, unshifted, want) in rows {
        let raw = RawKey {
            phase: KeyPhase::Press,
            key: Key::Character((*key).into()),
            code: *code,
            modifiers: *held,
            location: Location::Standard,
            text: None,
            unshifted: unshifted.map(|u| Key::Character(u.into())),
        };
        let stroke = stroke(&raw, Ime::Idle).unwrap_or_else(|| panic!("{name}: no stroke"));
        let got = match route(stroke, &keymap, FindBar::Closed) {
            Routed::Action(action) => Some(action),
            Routed::Ignored => None,
            Routed::Shell(_) => panic!("{name}: reached the program"),
        };
        assert_eq!(got, *want, "{name}");
    }
}

#[test]
fn altgr_text_is_typed_and_a_bare_ctrl_press_is_not() {
    use temor_vt::{KeyCode, KeyInput, Modifiers};

    use super::is_altgr_text;
    let ctrl_alt = Modifiers::CTRL.with(Modifiers::ALT);
    let cases = [
        (
            "AltGr @",
            KeyInput::press(KeyCode::Digit2, ctrl_alt, "@"),
            true,
        ),
        (
            "Ctrl+C",
            KeyInput::press(KeyCode::C, Modifiers::CTRL, ""),
            false,
        ),
        (
            "Ctrl+Alt+C with no text",
            KeyInput::press(KeyCode::C, ctrl_alt, ""),
            false,
        ),
        (
            "Alt+x",
            KeyInput::press(KeyCode::X, Modifiers::ALT, "x"),
            false,
        ),
    ];
    for (name, key, want) in cases {
        assert_eq!(is_altgr_text(&key), want, "{name}");
    }
}
