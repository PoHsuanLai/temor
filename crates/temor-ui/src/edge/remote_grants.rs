//! The answers the helper question once kept in `remote.json`, beside the session file. They live
//! in the consent book now; the file is only read, once, so what a person already answered
//! carries over. Keyed by the name the host was opened by.

use std::collections::BTreeMap;
use std::path::Path;

use temor_core::remote::Grant;

/// The file's name inside the state folder.
pub const FILE_NAME: &str = "remote.json";

/// Reads the old answers; none when the file is missing or unreadable.
pub fn load(dir: &Path) -> BTreeMap<String, Grant> {
    std::fs::read(dir.join(FILE_NAME))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_come_back_and_a_missing_or_broken_file_is_no_answers() {
        let dir = tempfile::tempdir().expect("a scratch folder");
        assert!(load(dir.path()).is_empty(), "no file, no answers");

        std::fs::write(
            dir.path().join(FILE_NAME),
            br#"{"prod": "allowed", "box": "refused"}"#,
        )
        .expect("writes");
        let want = BTreeMap::from([
            ("prod".to_owned(), Grant::Allowed),
            ("box".to_owned(), Grant::Refused),
        ]);
        assert_eq!(load(dir.path()), want);

        std::fs::write(dir.path().join(FILE_NAME), b"{ not json").expect("writes");
        assert!(load(dir.path()).is_empty(), "a broken file is no answers");
    }
}
