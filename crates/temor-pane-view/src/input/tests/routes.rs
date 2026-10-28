//! The find bar's keys, AltGr, and the routes that never reach the keymap.

use chordkit::Platform;
use dioxus::html::{Code, Key, Modifiers as Held};
use temor_panes::pane::keys::Action;
use temor_panes::vt::{KeyAction, KeyCode, KeyInput, Modifiers};

use super::super::keys::is_altgr_text;
use super::super::route::{FindBar, KittyKeys, Routed, Stroke, route};
use super::table::{OURS, Row, Went, ch, check, keymap_of, row, went, went_with};

/// The find bar's keys are the bar's only while it is open; closed, Escape and F3 are the
/// program's, and a Primary-form find chord is dropped like any other.
#[test]
fn find_keys_belong_to_the_bar_only_while_it_is_open() {
    let esc = |held| row("Escape", Key::Escape, Code::Escape, held, Went::Program);
    let close = Went::Action(Action::CloseFind);
    let rows: [(Platform, FindBar, Row, Went); 10] = [
        (OURS, FindBar::Open, esc(Held::empty()), close),
        (OURS, FindBar::Closed, esc(Held::empty()), Went::Program),
        (OURS, FindBar::Open, esc(Held::SHIFT), Went::Program),
        (
            OURS,
            FindBar::Open,
            row("⌘G", ch("g"), Code::KeyG, Held::SUPER, Went::Program),
            Went::Action(Action::FindNext),
        ),
        (
            OURS,
            FindBar::Closed,
            row("⌘G", ch("g"), Code::KeyG, Held::SUPER, Went::Program),
            Went::Dropped,
        ),
        (
            OURS,
            FindBar::Closed,
            row(
                "⌘⇧G",
                ch("g"),
                Code::KeyG,
                Held::SUPER | Held::SHIFT,
                Went::Program,
            ),
            Went::Dropped,
        ),
        (Platform::Windows, FindBar::Open, esc(Held::empty()), close),
        (
            Platform::Windows,
            FindBar::Closed,
            esc(Held::empty()),
            Went::Program,
        ),
        (
            Platform::Windows,
            FindBar::Open,
            row("F3", Key::F3, Code::F3, Held::empty(), Went::Program),
            Went::Action(Action::FindNext),
        ),
        (
            Platform::Windows,
            FindBar::Closed,
            row("F3", Key::F3, Code::F3, Held::empty(), Went::Program),
            Went::Program,
        ),
    ];
    for (platform, find, row, want) in &rows {
        let keymap = keymap_of(*platform);
        assert_eq!(
            went(&keymap, row, *find),
            *want,
            "{platform:?} {find:?} {}",
            row.name
        );
    }
}

/// AltGr types a character, however the platform reports it: the toolkit's flag, with or without
/// Ctrl and Alt (Windows adds both), or Ctrl and Alt alone with text. It is never a chord.
#[test]
fn altgr_characters_reach_the_program_on_every_platform() {
    let ctrl_alt = Held::CONTROL | Held::ALT;
    let typed = |name, held| Row {
        text: "@",
        ..row(name, ch("2"), Code::Digit2, held, Went::Program)
    };
    let rows = [
        typed("AltGr flag with Ctrl and Alt", ctrl_alt | Held::ALT_GRAPH),
        typed("AltGr flag alone", Held::ALT_GRAPH),
        typed("Ctrl and Alt with text", ctrl_alt),
        typed("AltGr flag with Shift", Held::ALT_GRAPH | Held::SHIFT),
    ];
    for platform in Platform::ALL {
        check(platform, FindBar::Closed, &rows);
    }
}

#[test]
fn composing_releases_and_unnamed_keys_keep_their_routes() {
    let keymap = keymap_of(OURS);
    let press = |composing, action, input: Option<chordkit::KeyInput>| Stroke {
        input,
        key: KeyInput {
            code: KeyCode::C,
            action,
            mods: Modifiers::SUPER,
            text: String::new(),
            unshifted: Some('c'),
            composing,
        },
    };
    let copy = chordkit::KeyInput::new(
        chordkit::Modifiers::of(chordkit::Modifier::Super),
        chordkit::Key::Char('c'),
    );
    // The input method owns the key while composing, even a bound one.
    let composing = press(true, KeyAction::Press, Some(copy));
    assert_eq!(route(composing, &keymap, FindBar::Closed), Routed::Ignored);
    // A release goes to the program (which may have asked for releases), never to an action.
    let release = press(false, KeyAction::Release, Some(copy));
    assert!(matches!(
        route(release, &keymap, FindBar::Closed),
        Routed::Shell(_)
    ));
    // A key chordkit has no name for is the program's.
    let unnamed = press(false, KeyAction::Press, None);
    assert!(matches!(
        route(unnamed, &keymap, FindBar::Closed),
        Routed::Shell(_)
    ));
}

#[test]
fn altgr_text_is_typed_and_a_bare_ctrl_press_is_not() {
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

/// A chord with no action is dropped, unless the program turned on the kitty keyboard flags: then
/// it asked for the chord. A chord with an action stays temor's either way.
#[test]
fn kitty_flags_let_unbound_primary_chords_through() {
    let keymap = keymap_of(OURS);
    let rows = [
        (
            row("⌘K", ch("k"), Code::KeyK, Held::SUPER, Went::Dropped),
            KittyKeys::Off,
            Went::Dropped,
        ),
        (
            row("⌘K", ch("k"), Code::KeyK, Held::SUPER, Went::Dropped),
            KittyKeys::On,
            Went::Program,
        ),
        (
            row("⌘C", ch("c"), Code::KeyC, Held::SUPER, Went::Dropped),
            KittyKeys::On,
            Went::Action(Action::Copy),
        ),
    ];
    for (row, kitty, want) in &rows {
        assert_eq!(
            went_with(&keymap, row, FindBar::Closed, *kitty),
            *want,
            "{kitty:?} {}",
            row.name
        );
    }
}
