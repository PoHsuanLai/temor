//! The last screen of each session in history (PLAN P23): `screens/<session>.bin` inside the
//! private state folder, mode 0600, none over [`SNAPSHOT_LIMIT`] bytes. A pane's screen is kept
//! under its session when its tab closes and when temor quits, so a session brought back at
//! launch, or reopened from its history page later, starts from what it showed. A screen goes
//! when its session leaves history, and only the newest [`MOST`] are kept.

use std::io;
use std::path::{Path, PathBuf};

use temor_core::session::SNAPSHOT_LIMIT;
use temor_schema::session::SessionId;

use super::private::{ensure_private_dir, write_private_file};

/// The folder inside the state folder.
const FOLDER: &str = "screens";

/// The most screens kept: the newest.
pub const MOST: usize = 200;

fn folder(state_dir: &Path) -> PathBuf {
    state_dir.join(FOLDER)
}

fn file(state_dir: &Path, id: SessionId) -> PathBuf {
    folder(state_dir).join(format!("{id}.bin"))
}

/// Keeps `bytes` as the screen of session `id`, in place of the one it had. A screen over the
/// limit is not kept, and the older one goes: it is no longer what the session showed.
pub fn write(state_dir: &Path, id: SessionId, bytes: &[u8]) -> io::Result<()> {
    if bytes.len() > SNAPSHOT_LIMIT {
        remove(state_dir, id);
        return Ok(());
    }
    ensure_private_dir(state_dir)?;
    ensure_private_dir(&folder(state_dir))?;
    write_private_file(&file(state_dir, id), bytes)
}

/// The screen kept for session `id`; it stays, for the next time.
pub fn read(state_dir: &Path, id: SessionId) -> Option<Vec<u8>> {
    let path = file(state_dir, id);
    let size = usize::try_from(std::fs::metadata(&path).ok()?.len()).ok()?;
    (size <= SNAPSHOT_LIMIT)
        .then(|| std::fs::read(&path).ok())
        .flatten()
}

/// Deletes the screen of session `id`, if one is kept.
pub fn remove(state_dir: &Path, id: SessionId) {
    let _ = std::fs::remove_file(file(state_dir, id));
}

/// Deletes the screens of the sessions `kept` says history no longer has, and all but the newest
/// [`MOST`] of the rest.
pub fn sweep(state_dir: &Path, kept: impl Fn(SessionId) -> bool) {
    let Ok(entries) = std::fs::read_dir(folder(state_dir)) else {
        return;
    };
    let mut screens = Vec::new();
    for path in entries.flatten().map(|entry| entry.path()) {
        let id = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .and_then(|stem| stem.parse::<SessionId>().ok());
        let screen = (path.extension().is_some_and(|ext| ext == "bin"))
            .then_some(id)
            .flatten();
        match screen {
            Some(id) if kept(id) => {
                let changed = std::fs::metadata(&path).and_then(|meta| meta.modified());
                screens.push((changed.ok(), path));
            }
            // Another session's, or a write cut short.
            _ => {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    screens.sort_by_key(|(changed, _)| std::cmp::Reverse(*changed));
    for (_, path) in screens.into_iter().skip(MOST) {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sessions_screen_is_kept_until_its_session_leaves_history() {
        let dir = tempfile::tempdir().expect("dir");
        let (one, two) = (SessionId::from_bits(1), SessionId::from_bits(2));
        write(dir.path(), one, b"first").expect("write");
        write(dir.path(), two, b"second").expect("write");
        assert_eq!(read(dir.path(), one).as_deref(), Some(b"first".as_slice()));
        assert_eq!(
            read(dir.path(), one).as_deref(),
            Some(b"first".as_slice()),
            "kept"
        );
        write(dir.path(), one, b"again").expect("write");
        assert_eq!(read(dir.path(), one).as_deref(), Some(b"again".as_slice()));
        std::fs::write(folder(dir.path()).join("stray.tmp"), "").expect("stray");
        sweep(dir.path(), |id| id == two);
        assert_eq!(read(dir.path(), one), None);
        assert_eq!(read(dir.path(), two).as_deref(), Some(b"second".as_slice()));
        assert_eq!(
            std::fs::read_dir(folder(dir.path())).expect("dir").count(),
            1
        );
    }

    #[test]
    fn a_screen_over_the_limit_is_not_kept_and_takes_the_old_one_with_it() {
        let dir = tempfile::tempdir().expect("dir");
        let id = SessionId::from_bits(7);
        write(dir.path(), id, b"small").expect("write");
        write(dir.path(), id, &vec![0; SNAPSHOT_LIMIT + 1]).expect("write");
        assert_eq!(read(dir.path(), id), None);
    }

    #[test]
    fn only_the_newest_screens_are_kept() {
        let dir = tempfile::tempdir().expect("dir");
        for bits in 0..(MOST as u64 + 3) {
            write(dir.path(), SessionId::from_bits(bits), b"s").expect("write");
        }
        sweep(dir.path(), |_| true);
        assert_eq!(
            std::fs::read_dir(folder(dir.path())).expect("dir").count(),
            MOST
        );
    }
}
