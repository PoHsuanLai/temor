//! Checkpoint files (PLAN §7): a pane's kept shell state and command history beside its snapshot,
//! one `<pane>.json` per pane in `checkpoints/` inside the private state folder, mode 0600.
//!
//! Pane ids restart from the live maximum after a restore, so a checkpoint is only good for the
//! launch that follows the quit that wrote it: [`take`] deletes what it reads and [`clear`]
//! removes every leftover once the launch has taken its panes.

use std::io;
use std::path::{Path, PathBuf};

use temor_blocks::session::Checkpoint;
use temor_core::vocab::ids::PaneId;

use super::private::{ensure_private_dir, write_private_file};

/// The folder inside the state folder.
const FOLDER: &str = "checkpoints";

/// The most bytes of a checkpoint file that are read.
const MOST_BYTES: u64 = 4 * 1024 * 1024;

fn folder(state_dir: &Path) -> PathBuf {
    state_dir.join(FOLDER)
}

fn file(state_dir: &Path, pane: PaneId) -> PathBuf {
    folder(state_dir).join(format!("{}.json", pane.0))
}

/// Stores `checkpoint` for `pane`.
pub fn write(state_dir: &Path, pane: PaneId, checkpoint: &Checkpoint) -> io::Result<()> {
    ensure_private_dir(state_dir)?;
    ensure_private_dir(&folder(state_dir))?;
    write_private_file(&file(state_dir, pane), checkpoint.encode().as_bytes())
}

/// The checkpoint stored for `pane`, which is deleted as it is read. `None` when there is none or
/// it cannot be read.
pub fn take(state_dir: &Path, pane: PaneId) -> Option<Checkpoint> {
    let path = file(state_dir, pane);
    let size = std::fs::metadata(&path).ok()?.len();
    let text = (size <= MOST_BYTES)
        .then(|| std::fs::read_to_string(&path).ok())
        .flatten();
    let _ = std::fs::remove_file(&path);
    Checkpoint::decode(&text?).ok()
}

/// Deletes the checkpoint stored for `pane`, if any (its tab was forgotten).
pub fn remove(state_dir: &Path, pane: PaneId) {
    let _ = std::fs::remove_file(file(state_dir, pane));
}

/// Deletes every stored checkpoint.
pub fn clear(state_dir: &Path) {
    let Ok(entries) = std::fs::read_dir(folder(state_dir)) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "json" || e == "tmp") {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use temor_blocks::session::Checkpoint;
    use temor_core::vocab::ids::PaneId;

    use super::{clear, remove, take, write};

    #[test]
    fn a_checkpoint_is_good_for_one_take() {
        let dir = tempfile::tempdir().expect("dir");
        let kept = Checkpoint {
            version: 1,
            cwd: Some("/a".into()),
            ..Checkpoint::default()
        };
        write(dir.path(), PaneId(3), &kept).expect("write");
        assert_eq!(take(dir.path(), PaneId(3)), Some(kept));
        assert_eq!(take(dir.path(), PaneId(3)), None, "taking deletes it");
        assert_eq!(take(dir.path(), PaneId(4)), None, "another pane has none");
    }

    #[test]
    fn a_forgotten_pane_and_a_clear_leave_nothing() {
        let dir = tempfile::tempdir().expect("dir");
        let kept = Checkpoint {
            version: 1,
            ..Checkpoint::default()
        };
        write(dir.path(), PaneId(1), &kept).expect("write");
        write(dir.path(), PaneId(2), &kept).expect("write");
        remove(dir.path(), PaneId(1));
        assert_eq!(take(dir.path(), PaneId(1)), None);
        clear(dir.path());
        assert_eq!(take(dir.path(), PaneId(2)), None);
    }

    #[test]
    fn a_damaged_file_is_nothing_and_is_deleted() {
        let dir = tempfile::tempdir().expect("dir");
        let kept = Checkpoint {
            version: 1,
            ..Checkpoint::default()
        };
        write(dir.path(), PaneId(5), &kept).expect("write");
        std::fs::write(dir.path().join("checkpoints/5.json"), "{broken").expect("damage");
        assert_eq!(take(dir.path(), PaneId(5)), None);
        assert!(!dir.path().join("checkpoints/5.json").exists());
    }
}
