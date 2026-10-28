//! Changing `keys.conf` from the Keyboard page: read the file as it is now, apply one change with
//! the pane's pure editors (`bind_key`, `reset_key`, `reset_all_keys`), write it, and build the
//! keymap the file now means. The keymap in force is built from the file each time, so a change
//! made meanwhile elsewhere is kept and a refusal names the action that holds the chord. The
//! panes follow the file (`hub::keys`); nothing here tells them.

use chordkit::{Chord, Context, Keymap, Overrides, Platform};
use temor_panes::pane::SettingsError;
use temor_panes::pane::keys::{
    Action, bind_key, build_keymap, portable_chord, reset_all_keys, reset_key,
};

use crate::edge::{Ambient, system_keys};
use crate::error::UiError;
use crate::settings::{overrides_of, read_keys_text, write_keys};

/// One change to the person's keys.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeyChange {
    /// The action answers to this chord alone.
    Bind(Action, Chord),
    /// The action answers to its own key again.
    Reset(Action),
    /// Every action does.
    ResetAll,
}

/// The keys as the file means them: the keymap in force, and the changes it holds.
#[derive(Debug, Clone)]
pub struct KeysView {
    pub keymap: Keymap,
    pub changed: Overrides,
}

impl KeysView {
    /// Whether the person has changed the key of `action`.
    pub fn changed(&self, action: Action) -> bool {
        action.to_chordkit().is_some_and(|named| {
            self.changed
                .entries()
                .iter()
                .any(|entry| entry.action == named)
        })
    }

    /// What `action` is pressed as in a terminal window, if it has a key.
    pub fn chord_of(&self, action: Action) -> Option<Chord> {
        let named = action.to_chordkit()?;
        self.keymap
            .chords_in(&named, Context::Terminal)
            .first()
            .copied()
    }
}

/// `text` with `change` made, judged by `keymap` (the keymap in force).
pub fn changed_text(
    text: &str,
    keymap: &Keymap,
    change: KeyChange,
) -> Result<String, SettingsError> {
    match change {
        KeyChange::Bind(action, chord) => {
            let written = portable_chord(keymap.platform(), chord);
            bind_key(text, keymap, action, &written)
        }
        KeyChange::Reset(action) => Ok(reset_key(text, action)),
        KeyChange::ResetAll => Ok(reset_all_keys(text)),
    }
}

/// The keymap `text` means on `platform`, with the desktop's own shortcut settings under it.
fn view_of(ambient: &Ambient, platform: Platform, text: &str) -> KeysView {
    let (changed, _) = overrides_of(text);
    let (keymap, _) = build_keymap(system_keys(platform, ambient).source(), platform, &changed);
    KeysView { keymap, changed }
}

/// The keys as `keys.conf` means them now.
pub fn load_view(ambient: &Ambient, platform: Platform) -> Result<KeysView, UiError> {
    let text = read_keys_text(&ambient.keys_path())?;
    Ok(view_of(ambient, platform, &text))
}

/// Makes `change` in `keys.conf`. A refused change (a chord another action holds, one the system
/// keeps) leaves the file as it was and says why, naming the holder.
pub fn change_keys(
    ambient: &Ambient,
    platform: Platform,
    change: KeyChange,
) -> Result<KeysView, UiError> {
    let path = ambient.keys_path();
    let text = read_keys_text(&path)?;
    let now = view_of(ambient, platform, &text);
    let next = changed_text(&text, &now.keymap, change)
        .map_err(|why| UiError::SettingsRefused(why.to_string()))?;
    write_keys(&path, &next)?;
    Ok(view_of(ambient, platform, &next))
}
