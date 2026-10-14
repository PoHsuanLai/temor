//! Click counting and the key stroke.

use std::time::Duration;

use dioxus::html::{Code, Key, Location, Modifiers as Held};
use ds::edit::raw_key::{KeyPhase, RawKey};
use temor_pane::text::CellPos;
use temor_vt::Ime;

use super::super::click::{Click, MULTI_CLICK, next_click};
use super::super::keys::{page_zoom, stroke};

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
    let raw = RawKey {
        phase: KeyPhase::Press,
        key: Key::Unidentified,
        code: Code::Unidentified,
        modifiers: Held::empty(),
        location: Location::Standard,
        text: None,
        unshifted: None,
    };
    assert!(stroke(&raw, Ime::Idle).is_none());
}

/// Ctrl or Super with `=`, `-` or `0` is the page's zoom and not the program's; with Shift it is
/// the program's (Ctrl+_ is undo), and a plain `0` is typed.
#[test]
fn the_zoom_keys_are_the_pages_but_not_with_shift() {
    let key = |code: Code, modifiers: Held| RawKey {
        phase: KeyPhase::Press,
        key: Key::Unidentified,
        code,
        modifiers,
        location: Location::Standard,
        text: None,
        unshifted: None,
    };
    for (code, modifiers, zooms) in [
        (Code::Equal, Held::CONTROL, true),
        (Code::Minus, Held::CONTROL, true),
        (Code::Digit0, Held::CONTROL, true),
        (Code::Digit0, Held::SUPER, true),
        (Code::Minus, Held::CONTROL | Held::SHIFT, false),
        (Code::Digit0, Held::empty(), false),
        (Code::Digit1, Held::CONTROL, false),
    ] {
        assert_eq!(
            page_zoom(&key(code, modifiers)),
            zooms,
            "{code:?} {modifiers:?}"
        );
    }
}
