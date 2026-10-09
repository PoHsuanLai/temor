//! Unit tests for click counting and the IME steps.

use std::time::Duration;

use temor_pane::text::CellPos;

use super::click::{Click, MULTI_CLICK, next_click};
use super::ime::{Composing, on_ime};
use ds::host::ime::ImeEvent;

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
fn a_preedit_starts_a_composition_and_its_commit_ends_it() {
    let (state, steps) = on_ime(
        Composing::Idle,
        ImeEvent::Preedit {
            text: "ni".into(),
            cursor: None,
        },
    );
    assert_eq!(state, Composing::Active);
    assert_eq!(steps.len(), 2);
    let (state, steps) = on_ime(state, ImeEvent::Commit("你".into()));
    assert_eq!(state, Composing::Idle);
    assert_eq!(steps.len(), 1);
}

#[test]
fn an_unidentified_key_is_not_sent_as_a_space() {
    use super::keys::{RawKey, Repeat, stroke};
    use dioxus::html::{Code, Key};
    use temor_vt::Ime;
    let raw = RawKey {
        key: &Key::Unidentified,
        code: &Code::Unidentified,
        held: dioxus::html::Modifiers::empty(),
        action: temor_vt::KeyAction::Press,
        repeating: Repeat::First,
        ime: Ime::Idle,
    };
    assert!(stroke(&raw).is_none());
}

/// The window's key events as the desktop delivers them, through the stroke and the router: a
/// ⌘ chord (Ctrl+Shift or Super form) runs its action, and everything else reaches the program.
#[test]
fn keys_as_the_desktop_sends_them_reach_the_action_or_the_program() {
    use dioxus::html::{Code, Key, Modifiers as Held};
    use temor_pane::keys::Keymap;
    use temor_vt::Ime;

    use temor_pane::keys::Action;
    use temor_vt::KeyAction;

    use super::keys::{RawKey, Repeat, stroke};
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
        (
            "unbound Ctrl+Shift+Z",
            Key::Character("Z".into()),
            Code::KeyZ,
            ctrl_shift,
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
            key,
            code,
            held: *held,
            action: KeyAction::Press,
            repeating: Repeat::First,
            ime: Ime::Idle,
        };
        let stroke = stroke(&raw).unwrap_or_else(|| panic!("{name}: no stroke"));
        // The find bar is open, so the find chords run (with it closed they do nothing).
        let got = match route(stroke, &keymap, FindBar::Open) {
            Routed::Action(action) => Some(action),
            Routed::Shell(_) => None,
            Routed::Ignored => panic!("{name}: ignored"),
        };
        assert_eq!(got, *want, "{name}");
    }
}
