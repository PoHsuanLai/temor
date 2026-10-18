//! A save that found `settings.toml` unreadable moves it aside before writing a new one
//! (ds-settings' `Saved::kept_at`). The person's old file is theirs: they are told, once, where it
//! went.

use std::path::Path;

/// What the person reads after a save set the unreadable file aside at `kept`.
pub fn kept_aside(kept: &Path) -> String {
    format!(
        "settings.toml could not be read, so a new one was written; the old file is kept as {}",
        kept.display()
    )
}
