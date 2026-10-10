//! The platforms, the routing rule `route` applies, and keymaps built from the conventions.

use chordkit::{Chord, Context, ConventionSource, Desktop, KeyInput, Keymap, Overrides, Platform};

use super::super::{Action, build_keymap};

pub(super) const OURS: Platform = Platform::Linux {
    desktop: Desktop::Ours,
};
pub(super) const KDE: Platform = Platform::Linux {
    desktop: Desktop::Kde,
};
pub(super) const GNOME: Platform = Platform::Linux {
    desktop: Desktop::Gnome,
};
pub(super) const OTHER: Platform = Platform::Linux {
    desktop: Desktop::Other,
};

/// Where a key goes in a terminal window, by the rule `route` applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Went {
    Ran(Action),
    Dropped,
    Program,
}

pub(super) fn press(text: &str) -> KeyInput {
    let chord: Chord = text.parse().expect("a valid chord in the table");
    KeyInput::new(chord.modifiers(), chord.key())
}

pub(super) fn keymap_for(platform: Platform) -> Keymap {
    let (keymap, issues) = build_keymap(&ConventionSource, platform, &Overrides::default());
    assert!(issues.is_empty(), "{platform:?}: {issues:?}");
    keymap
}

pub(super) fn went(keymap: &Keymap, input: &KeyInput) -> Went {
    match keymap
        .resolve(input, Context::Terminal)
        .and_then(|named| Action::from_chordkit(&named))
    {
        Some(action) => Went::Ran(action),
        None if keymap.is_primary_form(input, Context::Terminal) => Went::Dropped,
        None => Went::Program,
    }
}

pub(super) fn check(platform: Platform, rows: &[(&str, Went)]) {
    let keymap = keymap_for(platform);
    for (text, want) in rows {
        assert_eq!(went(&keymap, &press(text)), *want, "{platform:?} {text}");
    }
}

/// A keymap for any platform, ignoring registration problems (the "other" desktop has two).
pub(super) fn keymap_for_any(platform: Platform) -> Keymap {
    build_keymap(&ConventionSource, platform, &Overrides::default()).0
}
