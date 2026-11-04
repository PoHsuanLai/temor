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

use super::private::{ensure_private_dir, write_private_file};
use temor_blocks::session::SNAPSHOT_LIMIT;

/// The folder inside the state folder.
const FOLDER: &str = "snapshots";

fn folder(state_dir: &Path) -> PathBuf {
    state_dir.join(FOLDER)
}

fn file(state_dir: &Path, pane: PaneId) -> PathBuf {
    folder(state_dir).join(format!("{}.bin", pane.0))
}

/// Stores `snapshot` for `pane`. A snapshot over the limit is not stored, which is not an error:
/// there is just nothing to keep ([`Stored::TooLarge`]).
pub fn write(state_dir: &Path, pane: PaneId, bytes: &[u8]) -> io::Result<Stored> {
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
pub fn take(state_dir: &Path, pane: PaneId) -> Option<Vec<u8>> {
    let path = file(state_dir, pane);
    let size = std::fs::metadata(&path).ok()?.len();
    let read = match usize::try_from(size) {
        Ok(size) if size <= SNAPSHOT_LIMIT => std::fs::read(&path).ok(),
        _ => None,
    };
    let _ = std::fs::remove_file(&path);
    read
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use temor_blocks::session::SNAPSHOT_LIMIT;
    use temor_core::vocab::ids::PaneId;

    use super::{Stored, clear, remove, take, write};

    #[test]
    fn screens_are_private_capped_read_once_and_cleared() {
        let dir = tempfile::tempdir().expect("dir");
        let state = dir.path().join("temor");
        let small = vec![7; 64];
        assert_eq!(
            write(&state, PaneId(1), &small).expect("write"),
            Stored::Kept
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |p: &Path| std::fs::metadata(p).expect("meta").permissions().mode() & 0o777;
            assert_eq!(mode(&state.join("snapshots/1.bin")), 0o600);
            assert_eq!(mode(&state.join("snapshots")), 0o700);
            assert_eq!(mode(&state), 0o700);
        }
        // Over the cap nothing is stored.
        let big = vec![0; SNAPSHOT_LIMIT + 1];
        assert_eq!(
            write(&state, PaneId(2), &big).expect("write"),
            Stored::TooLarge
        );
        assert!(!state.join("snapshots/2.bin").exists());
        // Reading takes it: a second launch must not see an old pane's screen.
        assert_eq!(take(&state, PaneId(1)), Some(small.clone()));
        assert!(take(&state, PaneId(1)).is_none());
        // A forgotten pane's file goes, and so does anything left over.
        write(&state, PaneId(3), &small).expect("write");
        write(&state, PaneId(4), &small).expect("write");
        remove(&state, PaneId(3));
        assert!(!state.join("snapshots/3.bin").exists());
        clear(&state);
        assert!(!state.join("snapshots/4.bin").exists());
    }
}
