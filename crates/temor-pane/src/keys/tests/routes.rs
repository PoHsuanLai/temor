//! What each platform does with each key in a terminal (PLAN §6). Platforms are explicit values,
//! never the real environment.

use chordkit::{AltGr, Key, KeyInput, Modifier, Modifiers, Platform};

use super::super::Action;
use super::support::{GNOME, KDE, OURS, Went, check, keymap_for_any, went};

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
        ("Super+T", Ran(Action::NewTab)),
        // temor offers no separate "new" or font panel: ⌘N is free, dropped like any unbound ⌘.
        ("Super+N", Dropped),
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
        ("Ctrl+Shift+T", Ran(Action::NewTab)),
        ("Ctrl+Shift+W", Ran(Action::ClosePane)),
        ("Ctrl+Shift+D", Ran(Action::SplitRight)),
        // A Primary chord that uses Shift takes Alt as well, since Ctrl+Shift is the Primary.
        ("Ctrl+Shift+Alt+D", Ran(Action::SplitDown)),
        ("Ctrl+Shift+Alt+T", Ran(Action::ReopenTab)),
        // Equalize's fallback: Ctrl+Shift+= would be `+` under plain Ctrl, the program's.
        ("Ctrl+Shift+Alt+0", Ran(Action::Equalize)),
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
