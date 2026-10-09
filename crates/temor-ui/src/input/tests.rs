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
