//! A pane action's chord as quire draws it (a [`Shortcut`]), for menus, tooltips and the ⌘T bar
//! rows. The chord is the live one the keymap holds for this platform, so a person's own binding
//! shows too. Pure.

use chordkit::{DefaultChord, Keymap, PrimaryUse};
use ds_core::vocab::Shortcut;
use temor_pane::keys::Action;

/// What a person presses in a terminal window to run `action` (`Ctrl+Shift+D` on Windows or KDE,
/// where plain Ctrl is the program's). `None` when the action has no chord, or one quire has no
/// key cap for.
pub fn terminal_shortcut_of(keymap: &Keymap, action: Action) -> Option<Shortcut> {
    let named = action.to_chordkit()?;
    let chord = keymap
        .chords_in(&named, chordkit::Context::Terminal)
        .into_iter()
        .next()?;
    Shortcut::from_default_chord(DefaultChord::new(PrimaryUse::Unused, chord))
}
