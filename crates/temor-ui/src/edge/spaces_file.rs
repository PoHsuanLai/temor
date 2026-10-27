//! The Spaces file an older temor wrote (PLAN §7): quire's `Spaces<Project, Place>` as the kit
//! writes it, in the private state directory beside the session files. temor now keeps its Spaces
//! in the store (`hub::layout`) and reads this once, into it. The directory is injected. A write is
//! atomic and owner-only (0600); a file that is not JSON or holds no Space is set aside as
//! `spaces.json.bad`, never overwritten.

use std::io;
use std::path::Path;

use temor_core::space::TemorSpaces;

use super::session_file::set_aside;
use crate::error::UiError;
use crate::platform::{ensure_private_dir, write_private_file};

/// The file's name inside the state directory.
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
pub fn load_spaces(dir: &Path) -> Result<SpacesStartup, UiError> {
    let path = dir.join(SPACES_FILE);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            return Ok(SpacesStartup::NoFile);
        }
        Err(source) => return Err(UiError::Session(source)),
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
pub fn save_spaces(dir: &Path, spaces: &TemorSpaces) -> Result<(), UiError> {
    ensure_private_dir(dir).map_err(UiError::Session)?;
    let text = serde_json::to_string_pretty(spaces)
        .map_err(|e| UiError::Session(io::Error::other(e.to_string())))?;
    write_private_file(&dir.join(SPACES_FILE), format!("{text}\n").as_bytes())
        .map_err(UiError::Session)
}
