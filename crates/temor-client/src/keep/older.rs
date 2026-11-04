//! The files an older temor wrote: the session files (`session.json`, `session-<n>.json`) and
//! the Spaces file (`spaces.json`, quire's `Spaces<Project, Place>` as the kit writes it). temor
//! now keeps its rows and Spaces in the store ([`super::Kept`]) and reads these once, into it. The directory is
//! injected; the binary resolves it from the XDG state home. A damaged file is set aside
//! (`session.json.bad`), a newer one is left alone. The writers stay so tests can leave a file as
//! an older temor would.

use std::io;
use std::path::{Path, PathBuf};

use ds_style::space::list::SpaceId;
use temor_hosts::host::store::{HostsFile, read_hosts, splice_hosts};
use temor_layout::space::TemorSpaces;
use temor_layout::workspace::restore::{FreshWhy, Restored, decode, encode};
use temor_layout::workspace::{KeepClosed, Workspace};

use super::private::{ensure_private_dir, write_private_file};

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
    Restored(Box<Workspace>),
    /// The file could not be used; the window starts fresh (the reason is for the log).
    Fresh(FreshWhy),
}

/// Reads the first Space's session in `dir` (see [`load_for`]).
pub fn load(dir: &Path, keep: KeepClosed) -> io::Result<Startup> {
    load_for(dir, SpaceId(0), keep)
}

/// Reads the session of `space` in `dir`. A damaged file (not JSON, not valid text, no version)
/// is renamed to `<name>.bad`; a newer one is kept as it is, so a downgrade never destroys it.
pub fn load_for(dir: &Path, space: SpaceId, keep: KeepClosed) -> io::Result<Startup> {
    let path = dir.join(file_name(space));
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Startup::NoFile),
        Err(source) => return Err(source),
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
pub fn save(dir: &Path, ws: &Workspace) -> io::Result<()> {
    save_for(dir, SpaceId(0), ws)
}

/// Writes `ws` to the session file of `space` in `dir`, creating the directory (owner-only)
/// first. A file written by a newer build is never overwritten; a damaged one is set aside first.
pub fn save_for(dir: &Path, space: SpaceId, ws: &Workspace) -> io::Result<()> {
    write(dir, space, ws, None)
}

/// [`save_for`] the first Space, with the hosts (remembered answers, panes on hosts) kept under
/// the file's `hosts` key. Pane ids are unique across Spaces, so one record serves them all.
pub fn save_with_hosts(dir: &Path, ws: &Workspace, hosts: &HostsFile) -> io::Result<()> {
    write(dir, SpaceId(0), ws, Some(hosts))
}

/// The hosts kept in the first Space's session file in `dir`: nothing when there is no file or
/// it has none.
pub fn load_hosts(dir: &Path) -> HostsFile {
    std::fs::read(dir.join(FILE_NAME))
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .map(|text| read_hosts(&text))
        .unwrap_or_default()
}

fn write(dir: &Path, space: SpaceId, ws: &Workspace, hosts: Option<&HostsFile>) -> io::Result<()> {
    ensure_private_dir(dir)?;
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
    let failed = |e: temor_core::error::CoreError| io::Error::other(e.to_string());
    let mut text = encode(ws).map_err(failed)?;
    if let Some(hosts) = hosts {
        text = splice_hosts(&text, hosts).map_err(failed)?;
    }
    write_private_file(&path, text.as_bytes())
}

pub const SPACES_FILE: &str = "spaces.json";

/// What the file gave at start.
#[derive(Debug, Clone, PartialEq)]
pub enum SpacesStartup {
    /// No file: the first launch.
    NoFile,
    /// The saved Spaces.
    Loaded(TemorSpaces),
    /// The file could not be used and was set aside; the window starts with a first Space.
    Damaged,
}

/// Reads the Spaces in `dir`. The kit's reader is lenient (a Space with a damaged key takes its
/// default), so only a file that is not text, not JSON, or has no Space at all is damage.
pub fn load_spaces(dir: &Path) -> io::Result<SpacesStartup> {
    let path = dir.join(SPACES_FILE);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            return Ok(SpacesStartup::NoFile);
        }
        Err(source) => return Err(source),
    };
    let spaces = String::from_utf8(bytes)
        .ok()
        .and_then(|text| serde_json::from_str::<TemorSpaces>(&text).ok());
    match spaces {
        Some(spaces) => Ok(SpacesStartup::Loaded(spaces)),
        None => {
            set_aside(&path);
            Ok(SpacesStartup::Damaged)
        }
    }
}

/// Writes `spaces` to the Spaces file in `dir`, creating the directory (owner-only) first.
pub fn save_spaces(dir: &Path, spaces: &TemorSpaces) -> io::Result<()> {
    ensure_private_dir(dir)?;
    let text = serde_json::to_string_pretty(spaces).map_err(|e| io::Error::other(e.to_string()))?;
    write_private_file(&dir.join(SPACES_FILE), format!("{text}\n").as_bytes())
}
