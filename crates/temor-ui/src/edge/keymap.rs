//! The app's keymap, built once at start (PLAN §6): the desktop's own shortcut settings (or the
//! platform's conventions where there are none to read), then the person's key changes, then
//! temor's own actions with their defaults. The platform is read from the process environment
//! here and nowhere else; everything after it is the pure `build_keymap`.

use chordkit::{ConventionSource, Keymap, KeymapSource, Overrides, Platform, detect_process};
use temor_pane::keys::{KeyIssue, build_keymap};

use super::{Ambient, SystemKeys};

/// The keymap for the platform this process runs on, with its problems (see [`KeyIssue`]).
/// `CHORDKIT_PLATFORM` overrides the detection, as chordkit documents. The desktop's own
/// shortcut settings are read from where `ambient` says its config home is.
pub fn load_keymap(overrides: &Overrides, ambient: &Ambient) -> (Keymap, Vec<KeyIssue>) {
    let platform = detect_process();
    let keys = SystemKeys::choose(platform, &ambient.config_home(), &ambient.runtime_dir);
    build_keymap(&*keys.source(), platform, overrides)
}

/// The keymap for `platform` from its conventions alone, with `overrides` and temor's actions
/// laid over them. Tests name the platform; nothing here reads the environment or a file.
pub fn keymap_for(platform: Platform, overrides: &Overrides) -> (Keymap, Vec<KeyIssue>) {
    build_keymap(&ConventionSource as &dyn KeymapSource, platform, overrides)
}
