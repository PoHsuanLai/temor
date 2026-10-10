//! The hosts the person allowed or refused the helper on, kept in `remote.json` beside the
//! session file. Keyed by the name the host was opened by.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use temor_core::remote::Grant;

/// The file's name inside the state folder.
pub const FILE_NAME: &str = "remote.json";

/// Reads the answers; none when the file is missing or unreadable (every host is asked again).
pub fn load(dir: &Path) -> BTreeMap<String, Grant> {
    std::fs::read(dir.join(FILE_NAME))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Writes the answers beside the other state files, through a temporary file so a crash never
/// leaves half of one.
pub fn save(dir: &Path, grants: &BTreeMap<String, Grant>) -> std::io::Result<PathBuf> {
    let path = dir.join(FILE_NAME);
    let part = dir.join(format!("{FILE_NAME}.part"));
    let text = serde_json::to_vec_pretty(grants).map_err(std::io::Error::other)?;
    std::fs::write(&part, text)?;
    std::fs::rename(&part, &path)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_come_back_and_a_missing_or_broken_file_asks_again() {
        let dir = tempfile::tempdir().expect("a scratch folder");
        assert!(load(dir.path()).is_empty(), "no file, no answers");

        let mut grants = BTreeMap::new();
        grants.insert("prod".to_owned(), Grant::Allowed);
        grants.insert("box".to_owned(), Grant::Refused);
        save(dir.path(), &grants).expect("saves");
        assert_eq!(load(dir.path()), grants);

        std::fs::write(dir.path().join(FILE_NAME), b"{ not json").expect("writes");
        assert!(
            load(dir.path()).is_empty(),
            "a broken file is read as no answers"
        );
    }
}
