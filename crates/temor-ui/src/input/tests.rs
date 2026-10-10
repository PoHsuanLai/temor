//! Unit tests for click counting and the key stroke.

use std::time::Duration;

use chordkit::{Desktop, Keymap, Overrides, Platform};
use dioxus::html::{Code, Key, Location, Modifiers as Held};
use ds::edit::raw_key::{KeyPhase, RawKey};
use temor_pane::keys::Action;
use temor_pane::text::CellPos;
use temor_vt::{Ime, KeyAction, KeyCode, KeyInput, Modifiers};

use super::click::{Click, MULTI_CLICK, next_click};
use super::keys::stroke;
use super::route::{FindBar, Routed, Stroke, route};
use crate::edge::keymap_for;

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

/// What a key event did, by the router.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Went {
    Action(Action),
    Program,
    Dropped,
}

/// One key event and where it goes.
struct Row {
    name: &'static str,
    key: Key,
    code: Code,
    held: Held,
    unshifted: Option<&'static str>,
    text: &'static str,
    want: Went,
}

fn ch(text: &str) -> Key {
    Key::Character(text.into())
}

fn row(name: &'static str, key: Key, code: Code, held: Held, want: Went) -> Row {
    Row {
        name,
        key,
        code,
        held,
        unshifted: None,
        text: "",
        want,
    }
}

fn went(keymap: &Keymap, row: &Row, find: FindBar) -> Went {
    let raw = RawKey {
        phase: KeyPhase::Press,
        key: row.key.clone(),
        code: row.code,
        modifiers: row.held,
        location: Location::Standard,
        text: (!row.text.is_empty()).then(|| row.text.to_owned()),
        unshifted: row.unshifted.map(ch),
    };
    let stroke = stroke(&raw, Ime::Idle).unwrap_or_else(|| panic!("{}: no stroke", row.name));
    match route(stroke, keymap, find) {
        Routed::Action(action) => Went::Action(action),
        Routed::Shell(_) => Went::Program,
        Routed::Ignored => Went::Dropped,
    }
}

fn keymap_of(platform: Platform) -> Keymap {
    keymap_for(platform, &Overrides::default()).0
}

fn check(platform: Platform, find: FindBar, rows: &[Row]) {
    let keymap = keymap_of(platform);
    for row in rows {
        assert_eq!(
            went(&keymap, row, find),
            row.want,
            "{platform:?}: {}",
            row.name
        );
    }
}

const OURS: Platform = Platform::Linux {
    desktop: Desktop::Ours,
};
const KDE: Platform = Platform::Linux {
    desktop: Desktop::Kde,
};

/// Key events as our desktop delivers them, through the stroke and the router: ⌘ arrives as
/// Super, or as Ctrl+Shift when keycap remaps it for a terminal window. Shifted keys come with
/// the shifted character as the logical key, and still match their base key.
#[test]
fn our_desktop_sends_command_as_super_or_ctrl_shift() {
    let cs = Held::CONTROL | Held::SHIFT;
    let rows = [
        row(
            "⌘C as Ctrl+Shift+C",
            ch("C"),
            Code::KeyC,
            cs,
            Went::Action(Action::Copy),
        ),
        row(
            "⌘C as Super+C",
            ch("c"),
            Code::KeyC,
            Held::SUPER,
            Went::Action(Action::Copy),
        ),
        row(
            "⌘C as Meta+C",
            ch("c"),
            Code::KeyC,
            Held::META,
            Went::Action(Action::Copy),
        ),
        row(
            "⌘1, no unshifted key",
            ch("!"),
            Code::Digit1,
            cs,
            Went::Action(Action::SelectTab(1)),
        ),
        Row {
            unshifted: Some("1"),
            ..row(
                "⌘1, unshifted key given",
                ch("!"),
                Code::Digit1,
                cs,
                Went::Action(Action::SelectTab(1)),
            )
        },
        row(
            "⌘9, no unshifted key",
            ch("("),
            Code::Digit9,
            cs,
            Went::Action(Action::SelectTab(9)),
        ),
        row(
            "⌘W as Ctrl+Shift+W",
            ch("W"),
            Code::KeyW,
            cs,
            Went::Action(Action::ClosePane),
        ),
        row(
            "⌘N as Ctrl+Shift+N",
            ch("N"),
            Code::KeyN,
            cs,
            Went::Action(Action::NewTab),
        ),
        row(
            "⌘D as Ctrl+Shift+D",
            ch("D"),
            Code::KeyD,
            cs,
            Went::Action(Action::SplitRight),
        ),
        row(
            "⌘⇧D as Super+Shift+D",
            ch("D"),
            Code::KeyD,
            Held::SUPER | Held::SHIFT,
            Went::Action(Action::SplitDown),
        ),
        row(
            "⌃⌘= as Ctrl+Super+=",
            ch("="),
            Code::Equal,
            Held::CONTROL | Held::SUPER,
            Went::Action(Action::Equalize),
        ),
        // A ⌘ chord with no action is dropped, never a control byte for the program.
        row("⌘K has no action", ch("K"), Code::KeyK, cs, Went::Dropped),
        row(
            "Super+K has no action",
            ch("k"),
            Code::KeyK,
            Held::SUPER,
            Went::Dropped,
        ),
        row(
            "⌘T is the font panel's",
            ch("T"),
            Code::KeyT,
            cs,
            Went::Dropped,
        ),
        // The program's own keys.
        row(
            "Ctrl+C is the interrupt",
            ch("c"),
            Code::KeyC,
            Held::CONTROL,
            Went::Program,
        ),
        row(
            "Ctrl+D is end of input",
            ch("d"),
            Code::KeyD,
            Held::CONTROL,
            Went::Program,
        ),
        row(
            "Ctrl+1 is the desktop's",
            ch("1"),
            Code::Digit1,
            Held::CONTROL,
            Went::Program,
        ),
        row(
            "Ctrl+Space is the input method's",
            ch(" "),
            Code::Space,
            Held::CONTROL,
            Went::Program,
        ),
        // The desktop handles these before a window sees them.
        row(
            "Super alone",
            Key::Meta,
            Code::MetaLeft,
            Held::SUPER,
            Went::Program,
        ),
        row(
            "Super+Space",
            ch(" "),
            Code::Space,
            Held::SUPER,
            Went::Program,
        ),
        row("Super+Tab", Key::Tab, Code::Tab, Held::SUPER, Went::Program),
    ];
    check(OURS, FindBar::Closed, &rows);
}

#[test]
fn windows_and_kde_use_ctrl_shift_and_leave_plain_ctrl_and_super_alone() {
    let cs = Held::CONTROL | Held::SHIFT;
    let rows = [
        row(
            "Ctrl+Shift+C",
            ch("C"),
            Code::KeyC,
            cs,
            Went::Action(Action::Copy),
        ),
        row(
            "Ctrl+Shift+D",
            ch("D"),
            Code::KeyD,
            cs,
            Went::Action(Action::SplitRight),
        ),
        row(
            "Ctrl+Shift+Alt+D",
            ch("D"),
            Code::KeyD,
            cs | Held::ALT,
            Went::Action(Action::SplitDown),
        ),
        row(
            "Ctrl+Shift+1",
            ch("!"),
            Code::Digit1,
            cs,
            Went::Action(Action::SelectTab(1)),
        ),
        row(
            "Ctrl+Shift+K has no action",
            ch("K"),
            Code::KeyK,
            cs,
            Went::Dropped,
        ),
        row(
            "Ctrl+C is the interrupt",
            ch("c"),
            Code::KeyC,
            Held::CONTROL,
            Went::Program,
        ),
        row(
            "Ctrl+D is end of input",
            ch("d"),
            Code::KeyD,
            Held::CONTROL,
            Went::Program,
        ),
        row(
            "Ctrl+1",
            ch("1"),
            Code::Digit1,
            Held::CONTROL,
            Went::Program,
        ),
        row(
            "Super+C is not a copy here",
            ch("c"),
            Code::KeyC,
            Held::SUPER,
            Went::Program,
        ),
        row("Super+K", ch("k"), Code::KeyK, Held::SUPER, Went::Program),
    ];
    for platform in [Platform::Windows, KDE] {
        check(platform, FindBar::Closed, &rows);
    }
}

#[test]
fn macos_uses_command_only() {
    let rows = [
        row(
            "⌘C",
            ch("c"),
            Code::KeyC,
            Held::SUPER,
            Went::Action(Action::Copy),
        ),
        row(
            "⌘K has no action",
            ch("k"),
            Code::KeyK,
            Held::SUPER,
            Went::Dropped,
        ),
        row(
            "Ctrl+Shift+C is the program's",
            ch("C"),
            Code::KeyC,
            Held::CONTROL | Held::SHIFT,
            Went::Program,
        ),
        row("Ctrl+C", ch("c"), Code::KeyC, Held::CONTROL, Went::Program),
    ];
    check(Platform::MacOs, FindBar::Closed, &rows);
}

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
    let press = |ime, action, input: Option<chordkit::KeyInput>| Stroke {
        input,
        key: KeyInput {
            code: KeyCode::C,
            action,
            mods: Modifiers::SUPER,
            text: String::new(),
            unshifted: Some('c'),
            ime,
        },
    };
    let copy = chordkit::KeyInput::new(
        chordkit::Modifiers::of(chordkit::Modifier::Super),
        chordkit::Key::Char('c'),
    );
    // The input method owns the key while composing, even a bound one.
    let composing = press(Ime::Composing, KeyAction::Press, Some(copy));
    assert_eq!(route(composing, &keymap, FindBar::Closed), Routed::Ignored);
    // A release goes to the program (which may have asked for releases), never to an action.
    let release = press(Ime::Idle, KeyAction::Release, Some(copy));
    assert!(matches!(
        route(release, &keymap, FindBar::Closed),
        Routed::Shell(_)
    ));
    // A key chordkit has no name for is the program's.
    let unnamed = press(Ime::Idle, KeyAction::Press, None);
    assert!(matches!(
        route(unnamed, &keymap, FindBar::Closed),
        Routed::Shell(_)
    ));
}

#[test]
fn altgr_text_is_typed_and_a_bare_ctrl_press_is_not() {
    use super::super::is_altgr_text;
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
