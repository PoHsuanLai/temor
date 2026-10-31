//! What can be asked: the store's facts, as values. Each is a fact and nothing more; what a
//! sidebar or the history page makes of it is the window's.

use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use temor_core::folder::Entry;
use temor_schema::session::{At, RanCommand, Session, SessionId};
use temor_schema::url::TemorUrl;
use temor_store::{Found, Shelf, SqliteStore};

use crate::{ClientError, Query, Topics};

/// Sessions, newest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sessions {
    /// Only those that started in this folder or under it (an absolute path).
    pub folder: Option<String>,
    /// Only those that started before this, for the next page.
    pub before: Option<At>,
    /// At most this many.
    pub limit: usize,
}

impl Query for Sessions {
    type Output = Vec<Session>;

    const READS: Topics = Topics::SESSIONS;

    fn run(&self, store: &SqliteStore) -> Result<Vec<Session>, ClientError> {
        Ok(store.sessions(self.folder.as_deref(), self.before, self.limit)?)
    }
}

/// One session and its commands, in the order it ran them; `None` once it is forgotten.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Commands(pub SessionId);

impl Query for Commands {
    type Output = Option<(Session, Vec<RanCommand>)>;

    const READS: Topics = Topics::SESSIONS.union(Topics::BLOCKS);

    fn run(&self, store: &SqliteStore) -> Result<Self::Output, ClientError> {
        let Some(session) = store.session(self.0)? else {
            return Ok(None);
        };
        Ok(Some((session, store.commands_of(self.0)?)))
    }
}

/// The commands a history search finds, newest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct History {
    /// Text anywhere in a command or its folder, any case; none for every command.
    pub text: Option<String>,
    /// Only commands run in this folder or under it (an absolute path).
    pub folder: Option<String>,
    /// Only commands started before this, for the next page.
    pub before: Option<At>,
    /// At most this many.
    pub limit: usize,
}

impl Query for History {
    type Output = Vec<RanCommand>;

    const READS: Topics = Topics::BLOCKS;

    fn run(&self, store: &SqliteStore) -> Result<Vec<RanCommand>, ClientError> {
        Ok(store.search(&Found {
            text: self.text.as_deref(),
            folder: self.folder.as_deref(),
            before: self.before,
            limit: self.limit,
        })?)
    }
}

/// The places on a shelf (the pins, or the favourites), in the person's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnShelf(pub Shelf);

impl Query for OnShelf {
    type Output = Vec<TemorUrl>;

    const READS: Topics = Topics::PINS.union(Topics::FAVOURITES);

    fn run(&self, store: &SqliteStore) -> Result<Vec<TemorUrl>, ClientError> {
        Ok(store.shelf(self.0)?)
    }
}

/// What is in a folder on this computer, for its page: `None` when it cannot be read. A link to
/// a folder is listed as a folder, as Finder does. Read once when asked: a folder is not
/// watched, and nothing the store commits changes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folder(pub PathBuf);

impl Query for Folder {
    type Output = Option<Vec<Entry>>;

    const READS: Topics = Topics::empty();

    fn run(&self, _: &SqliteStore) -> Result<Self::Output, ClientError> {
        let Ok(items) = std::fs::read_dir(&self.0) else {
            return Ok(None);
        };
        let mut entries = Vec::new();
        for item in items {
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
        Ok(Some(entries))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_lists_its_files_and_folders_with_when_they_changed() {
        let dir = tempfile::tempdir().expect("a folder");
        std::fs::create_dir(dir.path().join("sub")).expect("a folder in it");
        std::fs::write(dir.path().join("a.txt"), "a").expect("a file");
        let store = SqliteStore::in_memory().expect("a store");
        let mut entries = Folder(dir.path().to_path_buf())
            .run(&store)
            .expect("read")
            .expect("readable");
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        let seen: Vec<(&str, bool)> = entries
            .iter()
            .map(|entry| (entry.name.as_str(), entry.folder))
            .collect();
        assert_eq!(seen, [("a.txt", false), ("sub", true)]);
        assert!(entries.iter().all(|entry| entry.modified.is_some()));
        let gone = Folder(dir.path().join("none")).run(&store).expect("read");
        assert_eq!(gone, None);
    }
}
