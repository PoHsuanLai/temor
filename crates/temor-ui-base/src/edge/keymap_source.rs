//! Where the desktop's own shortcut settings come from (PLAN §6): quire's system source
//! (`ds_blitz::system_key_source`), the one every quire app's window takes: kdeglobals on Plasma,
//! dconf on GNOME, the platform's conventions everywhere else, each watched for changes. temor
//! builds its own keymap on it (terminal keys, `keys.conf`), so it asks for the source with the
//! config home its `Ambient` names rather than the process's. Our desktop's keycap is not linked:
//! the desktop entry's `X-Keycap-Profile=keymap-aware` is all temor says to it.
//!
//! The person's own file, `keys.conf`, is watched here too ([`watch_file`]): a thread looks at its
//! modified time and calls back when it moves, and the window loads the keymap again
//! (`hub::keys`). Dropping the `Watch` stops the thread.

use std::path::Path;
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::{Duration, SystemTime};

use chordkit::{ChangeCallback, Platform, SourceError, Watch};
use ds::keys::KeySource;

use super::Ambient;

/// How often a watched file's modified time is looked at.
pub const LOOK_EVERY: Duration = Duration::from_secs(2);

/// The desktop's own shortcut settings for `platform`, under the config home `ambient` names (a
/// test's scratch folder, or the person's).
pub fn system_keys(platform: Platform, ambient: &Ambient) -> KeySource {
    let config_home = ambient.config_home().display().to_string();
    ds_blitz::system_key_source(platform, &|name| {
        (name == "XDG_CONFIG_HOME").then(|| config_home.clone())
    })
}

/// Calls `on_change` whenever the file at `path` is changed, made or removed, looking at its
/// modified time every `every`, until the returned `Watch` is dropped. The first look is taken
/// now, not on the thread, so a change made before the thread first runs still counts.
pub fn watch_file(
    path: &Path,
    every: Duration,
    on_change: ChangeCallback,
) -> Result<Watch, SourceError> {
    let (stop, alive) = channel::<()>();
    let path = path.to_path_buf();
    let seen = modified(&path);
    std::thread::Builder::new()
        .name("temor-keys-watch".to_owned())
        .spawn(move || look(&path, seen, every, &alive, &*on_change))
        .map_err(|err| {
            SourceError::Unsupported(format!(
                "couldn't watch the shortcut settings for changes: {err}"
            ))
        })?;
    // Dropping the sender ends the thread at its next look.
    Ok(Watch::holding(stop))
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
}

/// Calls `on_change` whenever `path`'s modified time (or its being there) differs from the last
/// look (`seen` at first), until `alive` is closed.
fn look(
    path: &Path,
    mut seen: Option<SystemTime>,
    every: Duration,
    alive: &Receiver<()>,
    on_change: &dyn Fn(),
) {
    loop {
        match alive.recv_timeout(every) {
            Err(RecvTimeoutError::Timeout) => {}
            Ok(()) | Err(RecvTimeoutError::Disconnected) => return,
        }
        let now = modified(path);
        if now != seen {
            seen = now;
            on_change();
        }
    }
}

#[cfg(test)]
#[path = "keymap_source_tests.rs"]
mod tests;
