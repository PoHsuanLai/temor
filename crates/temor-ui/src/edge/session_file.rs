//! The session file on disk (PLAN §7: a session is restored on launch). The directory is
//! injected; the binary resolves it from the XDG state home. A damaged file is set aside
//! (`session.json.bad`), a newer one is left alone, and every write is atomic and private.

use std::io;
use std::path::{Path, PathBuf};

use temor_core::workspace::restore::{FreshWhy, Restored, decode, encode};
use temor_core::workspace::{KeepClosed, Workspace};

use crate::error::UiError;
use crate::platform::{ensure_private_dir, write_private_file};

/// The file's name inside the state directory.
pub const FILE_NAME: &str = "session.json";

/// The state directory: `$XDG_STATE_HOME/temor`, else `~/.local/state/temor`. A relative or
/// missing value is not used, as the XDG spec says. `None` when neither is set (no saving then).
pub fn resolve_state_dir(xdg_state_home: Option<&Path>, home: Option<&Path>) -> Option<PathBuf> {
    let base = match xdg_state_home.filter(|p| p.is_absolute()) {
        Some(xdg) => xdg.to_path_buf(),
        None => home?.join(".local").join("state"),
    };
    Some(base.join("temor"))
}

/// What the file gave at start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Startup {
    /// No file: the first launch, or a launch after the file was removed.
    NoFile,
    /// The saved workspace.
    Restored(Workspace),
    /// The file could not be used; the window starts fresh (the reason is for the log).
    Fresh(FreshWhy),
}

/// Reads the session in `dir`. A damaged file (not JSON, not valid text, no version) is renamed
/// to `session.json.bad`; a newer one is kept as it is, so a downgrade never destroys it.
pub fn load(dir: &Path, keep: KeepClosed) -> Result<Startup, UiError> {
    let path = dir.join(FILE_NAME);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Startup::NoFile),
        Err(source) => return Err(UiError::Session(source)),
    };
    // Bytes that are not text cannot be a session file: they are damaged like any other.
    let decoded = match String::from_utf8(bytes) {
        Ok(text) => decode(&text, keep),
        Err(_) => Restored::Fresh(FreshWhy::Damaged),
    };
    match decoded {
        Restored::Loaded(ws) => Ok(Startup::Restored(ws)),
        Restored::Fresh(FreshWhy::NewerVersion(v)) => Ok(Startup::Fresh(FreshWhy::NewerVersion(v))),
        // Damaged, or any reason a later build adds: keep the evidence and start fresh.
        _ => {
            set_aside(&path);
            Ok(Startup::Fresh(FreshWhy::Damaged))
        }
    }
}

/// Renames a damaged file to `session.json.bad`: it keeps the evidence and lets the next save
/// write a clean file.
fn set_aside(path: &Path) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::rename(path, dir.join(format!("{FILE_NAME}.bad")));
    }
}

/// Writes `ws` to the session file in `dir`, creating the directory (owner-only) first. A
/// file written by a newer build is never overwritten; a damaged one is set aside first.
pub fn save(dir: &Path, ws: &Workspace) -> Result<(), UiError> {
    ensure_private_dir(dir).map_err(UiError::Session)?;
    let path = dir.join(FILE_NAME);
    if let Ok(existing) = std::fs::read(&path) {
        match String::from_utf8(existing) {
            Ok(text) => match decode(&text, KeepClosed::DEFAULT) {
                Restored::Fresh(FreshWhy::NewerVersion(_)) => return Ok(()),
                Restored::Fresh(FreshWhy::Damaged) => set_aside(&path),
                Restored::Loaded(_) => {}
                _ => set_aside(&path),
            },
            Err(_) => set_aside(&path),
        }
    }
    let text = encode(ws).map_err(|e| UiError::Session(io::Error::other(e.to_string())))?;
    write_private_file(&path, text.as_bytes()).map_err(UiError::Session)
}
