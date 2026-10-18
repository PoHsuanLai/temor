//! The ⌘-hover preview in the window (PLAN §10): holding ⌘ with the pointer on a file the shell
//! listed shows a small card beside it; letting go of ⌘ or typing puts it away, and the typing
//! still reaches the shell. Event dispatch only; the card reads a real file in a scratch folder.

use std::time::{Duration, Instant};

use dioxus::html::Modifiers;
use ds::base::vocab::ShortcutKey;
use ds::prelude::{Point, Px};
use ds_harness::{Harness, Input, PointerAction, PointerInput};
use temor_ui::view::probe::Probe;

use super::support::tools_present;
use super::viewer::listed;
use super::window::wait_until;

fn cards(h: &Harness) -> usize {
    h.html().matches(" preview\"").count()
}

fn pointer_at(h: &mut Harness, at: Point, held: Modifiers) {
    h.send(Input::Pointer(
        PointerInput::new(at, PointerAction::Move).with_mods(held),
    ));
}

/// ⌘ is Ctrl+Shift on this desktop, as the window's keymap reads it.
fn command() -> Modifiers {
    Modifiers::CONTROL | Modifiers::SHIFT
}

/// Moves down the pane with ⌘ held until a card shows; returns where the pointer rested. Every
/// row is tried, as the viewer scenarios do, because the font decides where the listing is.
fn sweep(h: &mut Harness) -> Point {
    let mut y = 14.0;
    loop {
        let at = Point {
            x: Px(272.0),
            y: Px(y),
        };
        pointer_at(h, at, command());
        let tried = Instant::now();
        while cards(h) == 0 && tried.elapsed() < Duration::from_millis(250) {
            h.advance(Duration::from_millis(25));
        }
        if cards(h) > 0 {
            return at;
        }
        y += 8.0;
        assert!(y < 320.0, "no card showed: {}", h.html());
    }
}

#[test]
fn command_hover_on_a_listed_file_shows_a_card_that_goes_when_command_does_or_a_key_is_typed() {
    if !tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = listed("peek-card", &probe);
    assert_eq!(cards(&h), 0, "no card before ⌘ is held");
    let at = sweep(&mut h);
    assert!(h.html().contains("a.txt preview"), "{}", h.html());

    // ⌘ let go with the pointer still: the card goes.
    // (a pixel over, so the move is a move)
    let nudged = |x: f32| Point { x: Px(x), y: at.y };
    pointer_at(&mut h, nudged(273.0), Modifiers::empty());
    h.advance(Duration::from_millis(100));
    assert_eq!(cards(&h), 0, "{}", h.html());

    // ⌘ again: it shows; a typed key puts it away and the shell hears the key.
    pointer_at(&mut h, nudged(274.0), command());
    let start = Instant::now();
    while cards(&h) == 0 {
        h.advance(Duration::from_millis(25));
        assert!(start.elapsed() < Duration::from_secs(10), "no card again");
    }
    h.send(Input::key(ShortcutKey::Char('x')));
    wait_until(&mut h, &probe, "the key to reach the shell", |s| {
        s.lines().any(|l| l.contains("$ x"))
    });
    assert_eq!(cards(&h), 0, "{}", h.html());
}
