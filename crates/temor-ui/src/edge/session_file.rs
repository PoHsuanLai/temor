//! The session file on disk (PLAN §7: a session is restored on launch). The directory is
//! injected; the binary resolves it from the XDG state home. A damaged file is set aside
//! (`session.json.bad`), a newer one is left alone, and every write is atomic and private.

use std::io;
use std::path::{Path, PathBuf};

use ds::components::app::spaces::SpaceId;
use temor_core::workspace::restore::{FreshWhy, Restored, decode, encode};
use temor_core::workspace::{KeepClosed, Workspace};

use crate::error::UiError;
use crate::platform::{ensure_private_dir, write_private_file};

/// The first Space's file name inside the state directory (the one a single-Space build wrote).
pub const FILE_NAME: &str = "session.json";

/// The session file of `space`: the first Space keeps `session.json`, so a session saved before
/// Spaces is that Space's; the others are `session-<id>.json`.
pub fn file_name(space: SpaceId) -> String {
    match space.0 {
        0 => FILE_NAME.to_owned(),
        n => format!("session-{n}.json"),
    }
}

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

/// Reads the first Space's session in `dir` (see [`load_for`]).
pub fn load(dir: &Path, keep: KeepClosed) -> Result<Startup, UiError> {
    load_for(dir, SpaceId(0), keep)
}

/// Reads the session of `space` in `dir`. A damaged file (not JSON, not valid text, no version)
/// is renamed to `<name>.bad`; a newer one is kept as it is, so a downgrade never destroys it.
pub fn load_for(dir: &Path, space: SpaceId, keep: KeepClosed) -> Result<Startup, UiError> {
    let path = dir.join(file_name(space));
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

/// Renames a damaged file to `<name>.bad`: it keeps the evidence and lets the next save write a
/// clean file.
pub(crate) fn set_aside(path: &Path) {
    if let (Some(dir), Some(name)) = (path.parent(), path.file_name()) {
        let mut bad = name.to_os_string();
        bad.push(".bad");
        let _ = std::fs::rename(path, dir.join(bad));
    }
}

/// Deletes the session file of a Space that was deleted, so a later Space with its number does
/// not start from it.
pub fn remove_for(dir: &Path, space: SpaceId) {
    let _ = std::fs::remove_file(dir.join(file_name(space)));
}

/// Writes `ws` to the first Space's session file (see [`save_for`]).
pub fn save(dir: &Path, ws: &Workspace) -> Result<(), UiError> {
    save_for(dir, SpaceId(0), ws)
}

/// Writes `ws` to the session file of `space` in `dir`, creating the directory (owner-only)
/// first. A file written by a newer build is never overwritten; a damaged one is set aside first.
pub fn save_for(dir: &Path, space: SpaceId, ws: &Workspace) -> Result<(), UiError> {
    ensure_private_dir(dir).map_err(UiError::Session)?;
    let path = dir.join(file_name(space));
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
