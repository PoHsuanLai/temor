//! The app's keymap, built once at start (PLAN §6): the desktop's conventions for the platform
//! the process runs on, then the person's key changes, then temor's own actions with their
//! defaults. The platform is read from the process environment here and nowhere else; everything
//! after it is the pure `build_keymap`.

use chordkit::{ConventionSource, Keymap, Overrides, Platform, detect_process};
use temor_pane::keys::{KeyIssue, build_keymap};

/// The keymap for the platform this process runs on, with its problems (see [`KeyIssue`]).
/// `CHORDKIT_PLATFORM` overrides the detection, as chordkit documents.
pub fn load_keymap(overrides: &Overrides) -> (Keymap, Vec<KeyIssue>) {
    keymap_for(detect_process(), overrides)
}

/// The keymap for `platform`: the platform's conventions, with `overrides` and temor's actions
/// laid over them. Tests name the platform; nothing here reads the environment.
///
/// TODO(keycap): once the desktop's own shortcut settings have a source (keycap on our desktop,
/// `chordkit-kde` on Plasma), pick it here by platform instead of the conventions alone.
pub fn keymap_for(platform: Platform, overrides: &Overrides) -> (Keymap, Vec<KeyIssue>) {
    build_keymap(&ConventionSource, platform, overrides)
}
