//! Snapshot sidecars (PLAN §7): a pane's screen and scrollback stored beside the session file
//! so a restart can start the pane from them. They live in `snapshots/` inside the private state
//! folder, one `<pane>.bin` per pane, mode 0600, none over [`SNAPSHOT_LIMIT`] bytes.
//!
//! Pane ids restart from the live maximum after a restore, so a sidecar is only good for the
//! launch that follows the quit that wrote it: [`take`] deletes what it reads and [`clear`]
//! removes every leftover once the launch has taken its panes.

use std::io;
use std::path::{Path, PathBuf};

use temor_core::vocab::ids::PaneId;
use temor_vt::Snapshot;

use crate::platform::{ensure_private_dir, write_private_file};
use crate::session::SNAPSHOT_LIMIT;

/// The folder inside the state folder.
const FOLDER: &str = "snapshots";

fn folder(state_dir: &Path) -> PathBuf {
    state_dir.join(FOLDER)
}

fn file(state_dir: &Path, pane: PaneId) -> PathBuf {
    folder(state_dir).join(format!("{}.bin", pane.0))
}

/// Stores `snapshot` for `pane`. A snapshot over the limit is not stored (and `Ok(false)` is not
/// an error: there is just nothing to keep).
pub fn write(state_dir: &Path, pane: PaneId, snapshot: &Snapshot) -> io::Result<Stored> {
    let bytes = snapshot.as_bytes();
    if bytes.len() > SNAPSHOT_LIMIT {
        return Ok(Stored::TooLarge);
    }
    ensure_private_dir(state_dir)?;
    ensure_private_dir(&folder(state_dir))?;
    write_private_file(&file(state_dir, pane), bytes)?;
    Ok(Stored::Kept)
}

/// Whether [`write`] kept the snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stored {
    /// It is on disk.
    Kept,
    /// It was over the limit and was not stored.
    TooLarge,
}

/// The snapshot stored for `pane`, which is deleted as it is read. `None` when there is none or
/// it cannot be read (a file over the limit is deleted unread).
pub fn take(state_dir: &Path, pane: PaneId) -> Option<Snapshot> {
    let path = file(state_dir, pane);
    let size = std::fs::metadata(&path).ok()?.len();
    let read = match usize::try_from(size) {
        Ok(size) if size <= SNAPSHOT_LIMIT => std::fs::read(&path).ok(),
        _ => None,
    };
    let _ = std::fs::remove_file(&path);
    read.map(Snapshot::from_bytes)
}

/// Deletes the snapshot stored for `pane`, if any (its tab was forgotten).
pub fn remove(state_dir: &Path, pane: PaneId) {
    let _ = std::fs::remove_file(file(state_dir, pane));
}

/// Deletes every stored snapshot.
pub fn clear(state_dir: &Path) {
    let Ok(entries) = std::fs::read_dir(folder(state_dir)) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "bin" || e == "tmp") {
            let _ = std::fs::remove_file(path);
        }
    }
}
