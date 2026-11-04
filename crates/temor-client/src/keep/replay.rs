//! What a new shell is handed: the aliases of the one it replaces, a script in the private state
//! folder that the shell reads once at its first prompt and deletes (`TEMOR_REPLAY` names it),
//! mode 0600; and the key its state reports carry (`TEMOR_REPORT_KEY`).

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::path::{Path, PathBuf};

use temor_shell::report::ReportKey;

use temor_core::vocab::ids::PaneId;

use super::private::{ensure_private_dir, write_private_file};

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

/// A new shell's report key: two words from the standard library's per-process random hash
/// seeds (the same source as the shell offer's load token).
pub fn fresh_key() -> ReportKey {
    let word = || RandomState::new().build_hasher().finish();
    ReportKey::from_words(word(), word())
}

/// Deletes every script a shell did not read (the last run ended before it did).
pub fn clear(state_dir: &Path) {
    let _ = std::fs::remove_dir_all(state_dir.join("replay"));
}
