//! The key rules (PLAN §6) over chordkit's keymap: what each platform does with each key in a
//! terminal, the action ids a person's file uses, and how key lines read. Platforms are explicit
//! values, never the real environment.

use chordkit::{
    AltGr, Chord, Context, ConventionSource, Desktop, Key, KeyInput, Keymap, KeymapSource,
    Modifier, Modifiers, Overrides, Platform, SourceError,
};

use super::{Action, build_keymap, declared, shortcut, terminal_shortcut};
use crate::error::SettingsError;

const OURS: Platform = Platform::Linux {
    desktop: Desktop::Ours,
};
const KDE: Platform = Platform::Linux {
    desktop: Desktop::Kde,
};
const GNOME: Platform = Platform::Linux {
    desktop: Desktop::Gnome,
};
const OTHER: Platform = Platform::Linux {
    desktop: Desktop::Other,
};

/// Where a key goes in a terminal window, by the rule `route` applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Went {
    Ran(Action),
    Dropped,
    Program,
}

fn press(text: &str) -> KeyInput {
    let chord: Chord = text.parse().expect("a valid chord in the table");
    KeyInput::new(chord.modifiers(), chord.key())
}

fn keymap_for(platform: Platform) -> Keymap {
    let (keymap, issues) = build_keymap(&ConventionSource, platform, &Overrides::default());
    assert!(issues.is_empty(), "{platform:?}: {issues:?}");
    keymap
}

fn went(keymap: &Keymap, input: &KeyInput) -> Went {
    match keymap
        .resolve(input, Context::Terminal)
        .and_then(|named| Action::from_chordkit(&named))
    {
        Some(action) => Went::Ran(action),
        None if keymap.is_primary_form(input, Context::Terminal) => Went::Dropped,
        None => Went::Program,
    }
}

fn check(platform: Platform, rows: &[(&str, Went)]) {
    let keymap = keymap_for(platform);
    for (text, want) in rows {
        assert_eq!(went(&keymap, &press(text)), *want, "{platform:?} {text}");
    }
}

#[test]
fn our_desktop_takes_super_and_the_ctrl_shift_form_keycap_sends() {
    use Went::{Dropped, Program, Ran};
    #[rustfmt::skip]
    let rows: &[(&str, Went)] = &[
        ("Super+C", Ran(Action::Copy)),
        ("Ctrl+Shift+C", Ran(Action::Copy)),
        ("Super+V", Ran(Action::Paste)),
        ("Super+F", Ran(Action::Find)),
        ("Super+G", Ran(Action::FindNext)),
        ("Super+Shift+G", Ran(Action::FindPrevious)),
        ("Super+N", Ran(Action::NewTab)),
        ("Super+W", Ran(Action::ClosePane)),
        ("Super+D", Ran(Action::SplitRight)),
        ("Ctrl+Shift+D", Ran(Action::SplitRight)),
        ("Super+Shift+D", Ran(Action::SplitDown)),
        ("Super+Alt+Left", Ran(Action::FocusLeft)),
        ("Super+Alt+Down", Ran(Action::FocusDown)),
        ("Super+Up", Ran(Action::PreviousPrompt)),
        ("Super+Down", Ran(Action::NextPrompt)),
        ("Super+Shift+Enter", Ran(Action::ToggleZoom)),
        ("Ctrl+Super+=", Ran(Action::Equalize)),
        ("Super+Shift+T", Ran(Action::ReopenTab)),
        ("Super+1", Ran(Action::SelectTab(1))),
        ("Ctrl+Shift+9", Ran(Action::SelectTab(9))),
        // The system's Cancel; `route` decides whether a find bar is there to close.
        ("Super+.", Ran(Action::CloseFind)),
        // Plain Ctrl is the program's, whatever it is.
        ("Ctrl+D", Program),
        ("Ctrl+C", Program),
        ("Escape", Program),
        // Ctrl+digit is the desktop's Space switch: never bound here, never typed at the program
        // as anything but itself.
        ("Ctrl+1", Program),
        ("Ctrl+9", Program),
        ("Ctrl+Alt+1", Program),
        // The desktop's own chords are handled before they reach a window.
        ("Super+Space", Program),
        ("Super+Tab", Program),
        // A ⌘ chord with no action here is dropped, not sent as a control byte.
        ("Super+K", Dropped),
        ("Ctrl+Shift+K", Dropped),
        ("Ctrl+Shift+;", Dropped),
        // The system's actions that a terminal has no use for are dropped the same way.
        ("Super+X", Dropped),
    ];
    check(OURS, rows);
}

#[test]
fn the_ctrl_shift_platforms_keep_plain_ctrl_for_the_program() {
    use Went::{Dropped, Program, Ran};
    #[rustfmt::skip]
    let rows: &[(&str, Went)] = &[
        ("Ctrl+Shift+C", Ran(Action::Copy)),
        ("Ctrl+Shift+V", Ran(Action::Paste)),
        ("Ctrl+Shift+F", Ran(Action::Find)),
        ("Ctrl+Shift+N", Ran(Action::NewTab)),
        ("Ctrl+Shift+W", Ran(Action::ClosePane)),
        ("Ctrl+Shift+D", Ran(Action::SplitRight)),
        // A Primary chord that uses Shift takes Alt as well, since Ctrl+Shift is the Primary.
        ("Ctrl+Shift+Alt+D", Ran(Action::SplitDown)),
        ("Ctrl+Shift+Alt+T", Ran(Action::ReopenTab)),
        ("Ctrl+Shift+Alt+Enter", Ran(Action::ToggleZoom)),
        ("Ctrl+Shift+Alt+Left", Ran(Action::FocusLeft)),
        ("Ctrl+Shift+Up", Ran(Action::PreviousPrompt)),
        ("Ctrl+Shift+1", Ran(Action::SelectTab(1))),
        ("Ctrl+Shift+9", Ran(Action::SelectTab(9))),
        ("Ctrl+D", Program),
        ("Ctrl+C", Program),
        ("Ctrl+1", Program),
        ("Ctrl+9", Program),
        ("Ctrl+Alt+Left", Program),
        // Super is not the Primary here: its chords belong to the OS or the program.
        ("Super+D", Program),
        ("Super+K", Program),
        ("Ctrl+Shift+K", Dropped),
        ("Ctrl+Shift+;", Dropped),
    ];
    for platform in [Platform::Windows, KDE, GNOME] {
        check(platform, rows);
    }
}

#[test]
fn the_find_keys_follow_the_platform() {
    use Went::Ran;
    for platform in [Platform::Windows, KDE] {
        check(
            platform,
            &[
                ("F3", Ran(Action::FindNext)),
                ("Shift+F3", Ran(Action::FindPrevious)),
                ("Escape", Ran(Action::CloseFind)),
            ],
        );
    }
    check(
        GNOME,
        &[
            ("Ctrl+Shift+G", Ran(Action::FindNext)),
            ("Ctrl+Shift+Alt+G", Ran(Action::FindPrevious)),
        ],
    );
}

#[test]
fn macos_takes_super_only() {
    use Went::{Dropped, Program, Ran};
    check(
        Platform::MacOs,
        &[
            ("Super+D", Ran(Action::SplitRight)),
            ("Super+Shift+D", Ran(Action::SplitDown)),
            ("Super+1", Ran(Action::SelectTab(1))),
            ("Super+K", Dropped),
            ("Ctrl+D", Program),
            ("Ctrl+Shift+D", Program),
            ("Ctrl+1", Program),
        ],
    );
}

#[test]
fn altgr_characters_type_on_every_platform() {
    let held = |mods: &[Modifier], key: Key| {
        KeyInput::new(mods.iter().copied().collect::<Modifiers>(), key).with_alt_gr(AltGr::Held)
    };
    let inputs = [
        held(&[Modifier::Ctrl, Modifier::Alt], Key::Char('2')),
        held(&[Modifier::Ctrl, Modifier::Alt], Key::Char('q')),
        held(&[Modifier::Alt], Key::Char('e')),
        held(
            &[Modifier::Ctrl, Modifier::Alt, Modifier::Shift],
            Key::Char('d'),
        ),
        held(&[Modifier::Super], Key::Char('c')),
    ];
    for platform in Platform::ALL {
        let keymap = keymap_for_any(platform);
        for input in &inputs {
            assert_eq!(
                went(&keymap, input),
                Went::Program,
                "{platform:?} {input:?}"
            );
        }
    }
}

/// A keymap for any platform, ignoring registration problems (the "other" desktop has two).
fn keymap_for_any(platform: Platform) -> Keymap {
    build_keymap(&ConventionSource, platform, &Overrides::default()).0
}

#[test]
fn every_declared_default_registers_except_where_a_standard_chord_has_it() {
    for platform in [Platform::MacOs, Platform::Windows, KDE, GNOME, OURS] {
        let (_, issues) = build_keymap(&ConventionSource, platform, &Overrides::default());
        assert!(issues.is_empty(), "{platform:?}: {issues:?}");
    }
    // Ctrl+Alt+Left and Right are the common conventions' Space switch.
    let (keymap, issues) = build_keymap(&ConventionSource, OTHER, &Overrides::default());
    let mentioned = |id: &str| {
        issues
            .iter()
            .filter(|(line, issue)| {
                *line == 0 && matches!(issue, SettingsError::Key(m) if m.contains(id))
            })
            .count()
    };
    assert_eq!(issues.len(), 2, "{issues:?}");
    assert_eq!(
        (
            mentioned("temor.focus-left"),
            mentioned("temor.focus-right")
        ),
        (1, 1)
    );
    // The refusal costs those two actions only.
    assert_eq!(
        went(&keymap, &press("Ctrl+Shift+Alt+Up")),
        Went::Ran(Action::FocusUp)
    );
}

#[test]
fn action_ids_are_the_ones_a_keys_file_writes() {
    #[rustfmt::skip]
    let rows: &[(Action, &str)] = &[
        (Action::Copy, "copy"),
        (Action::Paste, "paste"),
        (Action::Find, "find"),
        (Action::FindNext, "find-next"),
        (Action::FindPrevious, "find-previous"),
        (Action::NewTab, "new"),
        (Action::ClosePane, "close"),
        (Action::SplitRight, "temor.split-right"),
        (Action::SplitDown, "temor.split-down"),
        (Action::FocusLeft, "temor.focus-left"),
        (Action::FocusRight, "temor.focus-right"),
        (Action::FocusUp, "temor.focus-up"),
        (Action::FocusDown, "temor.focus-down"),
        (Action::PreviousPrompt, "temor.previous-prompt"),
        (Action::NextPrompt, "temor.next-prompt"),
        (Action::ToggleZoom, "temor.zoom"),
        (Action::Equalize, "temor.equalize"),
        (Action::ReopenTab, "temor.reopen-tab"),
        (Action::SelectTab(1), "temor.tab-1"),
        (Action::SelectTab(9), "temor.tab-9"),
        (Action::SelectSpace(3), "switch-to-space-3"),
    ];
    for (action, id) in rows {
        let named = action.to_chordkit().expect("bound by a keymap entry");
        assert_eq!(named.id(), *id);
        assert_eq!(Action::from_chordkit(&named), Some(*action), "{id}");
    }
    assert_eq!(Action::CloseFind.to_chordkit(), None);
    assert_eq!(Action::SelectTab(0).to_chordkit(), None);
    assert_eq!(Action::SelectTab(10).to_chordkit(), None);
    for id in [
        "select-all",
        "temor.nope",
        "temor.tab-0",
        "temor.tab-10",
        "mail.split-right",
    ] {
        let named: chordkit::Action = id.parse().expect("an action id");
        assert_eq!(Action::from_chordkit(&named), None, "{id}");
    }
}

#[test]
fn the_declared_defaults_are_temors_own_only() {
    let ids: Vec<String> = declared()
        .iter()
        .filter_map(|(action, _)| action.to_chordkit())
        .map(|named| named.id())
        .collect();
    assert_eq!(ids.len(), declared().len());
    assert!(ids.iter().all(|id| id.starts_with("temor.")), "{ids:?}");
}

#[test]
fn a_persons_changes_win_and_a_refused_one_keeps_the_default() {
    let (overrides, bad) =
        Overrides::parse("temor.split-right = Primary+Alt+N\ncopy = none\ntemor.zoom = Super+K\n");
    assert!(bad.is_empty(), "{bad:?}");
    let (keymap, issues) = build_keymap(&ConventionSource, Platform::Windows, &overrides);
    // Windows keeps every Windows-key chord for itself: line 3 is refused, in words.
    assert!(
        matches!(issues.as_slice(), [(3, SettingsError::Key(_))]),
        "{issues:?}"
    );
    use Went::{Dropped, Ran};
    for (text, want) in [
        ("Ctrl+Shift+Alt+N", Ran(Action::SplitRight)),
        ("Ctrl+Shift+D", Dropped),
        ("Ctrl+Shift+C", Dropped),
        ("Ctrl+Shift+Alt+Enter", Ran(Action::ToggleZoom)),
    ] {
        assert_eq!(went(&keymap, &press(text)), want, "{text}");
    }
}

/// A source whose settings cannot be read.
#[derive(Debug)]
struct Unreadable;

impl KeymapSource for Unreadable {
    fn load(&self, _platform: Platform) -> Result<Keymap, SourceError> {
        Err(SourceError::Unsupported("no shortcut settings here".into()))
    }
}

#[test]
fn an_unreadable_source_leaves_the_conventions() {
    let (keymap, issues) = build_keymap(&Unreadable, KDE, &Overrides::default());
    assert!(
        matches!(issues.as_slice(), [(0, SettingsError::Key(m))] if m.contains("no shortcut settings here")),
        "{issues:?}"
    );
    assert_eq!(
        went(&keymap, &press("Ctrl+Shift+C")),
        Went::Ran(Action::Copy)
    );
}

#[test]
fn shortcuts_are_shown_the_way_the_platform_writes_them() {
    let rows: &[(Platform, Action, Option<&str>, Option<&str>)] = &[
        (OURS, Action::Copy, Some("⌘C"), Some("⌘C")),
        (OURS, Action::SplitDown, Some("⇧⌘D"), Some("⇧⌘D")),
        (
            Platform::Windows,
            Action::SplitRight,
            Some("Ctrl+D"),
            Some("Ctrl+Shift+D"),
        ),
        (
            Platform::Windows,
            Action::SplitDown,
            Some("Ctrl+Shift+D"),
            Some("Ctrl+Alt+Shift+D"),
        ),
        (OURS, Action::SelectSpace(1), Some("⌃1"), None),
        (OURS, Action::CloseFind, None, None),
    ];
    for (platform, action, menu, terminal) in rows {
        let keymap = keymap_for(*platform);
        assert_eq!(
            shortcut(&keymap, *action).as_deref(),
            *menu,
            "{platform:?} {action:?}"
        );
        assert_eq!(
            terminal_shortcut(&keymap, *action).as_deref(),
            *terminal,
            "{platform:?} {action:?}"
        );
    }
}
