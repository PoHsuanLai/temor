//! The aliases a new shell is given: a script in the private state folder that the shell reads
//! once at its first prompt and deletes (`TEMOR_REPLAY` names it), mode 0600.

use std::path::{Path, PathBuf};

use temor_core::vocab::ids::PaneId;

use crate::platform::{ensure_private_dir, write_private_file};

/// Stores `script` for `pane`'s next shell and returns where. `None` when there is nothing to
/// give or it could not be stored.
pub fn write(state_dir: &Path, pane: PaneId, script: &str) -> Option<PathBuf> {
    if script.is_empty() {
        return None;
    }
    let folder = state_dir.join("replay");
    ensure_private_dir(state_dir).ok()?;
    ensure_private_dir(&folder).ok()?;
    let path = folder.join(format!("{}.zsh", pane.0));
    write_private_file(&path, script.as_bytes()).ok()?;
    Some(path)
}

/// Deletes every script a shell did not read (the last run ended before it did).
pub fn clear(state_dir: &Path) {
    let _ = std::fs::remove_dir_all(state_dir.join("replay"));
}
