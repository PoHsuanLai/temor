//! What is in a folder on this computer, for its page (`temor_core::folder` lists it). A link to a
//! folder is listed as a folder, as Finder does.

use std::path::Path;
use std::time::UNIX_EPOCH;

use temor_core::folder::Entry;
use temor_schema::session::At;

/// The entries of `folder`; an error when it cannot be read.
pub fn read_folder(folder: &Path) -> std::io::Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for item in std::fs::read_dir(folder)? {
        let Ok(item) = item else { continue };
        let path = item.path();
        let meta = std::fs::metadata(&path).or_else(|_| item.metadata());
        let (folder, modified) = match meta {
            Ok(meta) => (meta.is_dir(), meta.modified().ok()),
            Err(_) => (false, None),
        };
        let modified = modified
            .and_then(|at| at.duration_since(UNIX_EPOCH).ok())
            .and_then(|since| i64::try_from(since.as_millis()).ok())
            .map(At);
        entries.push(Entry {
            name: item.file_name().to_string_lossy().into_owned(),
            folder,
            modified,
        });
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_lists_its_files_and_folders_with_when_they_changed() {
        let dir = std::env::temp_dir().join(format!("temor-folder-read-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).expect("a folder");
        std::fs::write(dir.join("a.txt"), "a").expect("a file");
        let mut entries = read_folder(&dir).expect("read");
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        let seen: Vec<(&str, bool)> = entries
            .iter()
            .map(|entry| (entry.name.as_str(), entry.folder))
            .collect();
        assert_eq!(seen, [("a.txt", false), ("sub", true)]);
        assert!(entries.iter().all(|entry| entry.modified.is_some()));
        assert!(read_folder(&dir.join("none")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
