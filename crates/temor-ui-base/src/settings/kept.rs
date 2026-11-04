//! A save that found `settings.toml` unreadable moves it aside before writing a new one
//! (ds-settings' `Saved::kept_at`). The person's old file is theirs: they are told, once, where it
//! went.

use std::path::Path;

/// What the person reads after a save set the unreadable file aside at `kept`.
pub fn kept_aside(kept: &Path) -> String {
    format!(
        "Your settings couldn't be read, so temor started a fresh settings file. The old one is \
         kept as {}.",
        kept.display()
    )
}
