//! The `~/.ssh/config` host list, read again only when something it read looks changed. The
//! command bar asks for the hosts on every keystroke; parsing the files each time is wasted work.
//!
//! What counts as "something it read" is every file the load opened (the config and each file an
//! `Include` pulled in) and every folder it listed for a pattern (the folder's modified time moves
//! when an included file is added or removed). An included file edited in place is noticed by its
//! own time.

use std::cell::RefCell;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use temor_ssh::{ConfigFiles, ConfigPaths, FsConfigFiles, HostList, load_host_list};

/// A path and when it was last modified; `None` for one that is not there.
type Seen = (PathBuf, Option<SystemTime>);

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
}

/// The real files, noting each path it is asked for with its modified time just before the read,
/// so a change made while the list is being parsed still shows up as a change next time.
struct Noting<'a> {
    inner: &'a dyn ConfigFiles,
    stat: &'a dyn Fn(&Path) -> Option<SystemTime>,
    seen: RefCell<Vec<Seen>>,
}

impl Noting<'_> {
    fn note(&self, path: &Path) {
        self.seen
            .borrow_mut()
            .push((path.to_owned(), (self.stat)(path)));
    }
}

impl ConfigFiles for Noting<'_> {
    fn read(&self, path: &Path) -> io::Result<String> {
        self.note(path);
        self.inner.read(path)
    }

    fn list(&self, dir: &Path) -> io::Result<Vec<PathBuf>> {
        self.note(dir);
        self.inner.list(dir)
    }
}

/// What a kept list was read from.
#[derive(Debug, Clone)]
struct Kept {
    config: PathBuf,
    seen: Vec<Seen>,
    list: HostList,
}

impl Kept {
    /// Whether this is the list for `config` and nothing it read has changed under `stat`.
    fn holds(&self, config: &Path, stat: &dyn Fn(&Path) -> Option<SystemTime>) -> bool {
        self.config == config && self.seen.iter().all(|(path, was)| stat(path) == *was)
    }
}

/// The last list read and what it was read from. Cloning shares the cache.
#[derive(Debug, Clone, Default)]
pub(super) struct HostCache(Arc<Mutex<Option<Kept>>>);

impl HostCache {
    /// The list for the config at `config`: the kept one while every path it was read from keeps
    /// its modified time under `stat`, else `load`'s (with the paths it read), kept.
    pub(super) fn get(
        &self,
        config: &Path,
        stat: &dyn Fn(&Path) -> Option<SystemTime>,
        load: impl FnOnce() -> (HostList, Vec<Seen>),
    ) -> HostList {
        let mut slot = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(kept) = slot.as_ref()
            && kept.holds(config, stat)
        {
            return kept.list.clone();
        }
        let (list, seen) = load();
        *slot = Some(Kept {
            config: config.to_owned(),
            seen,
            list: list.clone(),
        });
        list
    }

    /// The list for the config at `paths`, from disk only when it, a file it includes, or a
    /// folder it lists changed.
    pub(super) fn list(&self, paths: &ConfigPaths) -> HostList {
        self.get(&paths.config, &modified, || {
            let files = Noting {
                inner: &FsConfigFiles,
                stat: &modified,
                seen: RefCell::new(Vec::new()),
            };
            let list = load_host_list(&files, paths);
            (list, files.seen.into_inner())
        })
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::collections::BTreeMap;
    use std::fs;
    use std::time::Duration;

    use super::*;

    fn at(secs: u64) -> Option<SystemTime> {
        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(secs))
    }

    /// The list is parsed again only when a path it read changes; the rows are
    /// (what changed in the fake disk, parses so far).
    #[test]
    fn the_files_are_parsed_again_only_when_one_it_read_changes() {
        let cache = HostCache::default();
        let loads = Cell::new(0);
        let mut disk: BTreeMap<&str, u64> = BTreeMap::from([("/c", 1), ("/inc", 1), ("/other", 1)]);
        #[allow(clippy::type_complexity)]
        let rows: [(&str, Option<(&'static str, u64)>, i32); 6] = [
            ("first read", None, 1),
            ("nothing changed", None, 1),
            ("the config edited", Some(("/c", 2)), 2),
            ("an included file edited in place", Some(("/inc", 2)), 3),
            ("a file it never read", Some(("/other", 9)), 3),
            ("the included file touched again", Some(("/inc", 3)), 4),
        ];
        for (what, change, want) in rows {
            if let Some((path, secs)) = change {
                disk.insert(path, secs);
            }
            let stat = |path: &Path| disk.get(path.to_str().unwrap_or("")).and_then(|s| at(*s));
            cache.get(Path::new("/c"), &stat, || {
                loads.set(loads.get() + 1);
                let seen = ["/c", "/inc"]
                    .map(|p| (PathBuf::from(p), stat(Path::new(p))))
                    .to_vec();
                (HostList::default(), seen)
            });
            assert_eq!(loads.get(), want, "{what}");
        }
    }

    #[test]
    fn a_different_config_is_a_different_list() {
        let cache = HostCache::default();
        let loads = Cell::new(0);
        let stat = |_: &Path| at(1);
        for config in ["/a", "/a", "/b"] {
            cache.get(Path::new(config), &stat, || {
                loads.set(loads.get() + 1);
                (HostList::default(), Vec::new())
            });
        }
        assert_eq!(loads.get(), 2);
    }

    #[test]
    fn an_included_file_edited_on_disk_is_seen_and_a_missing_config_gives_an_empty_list() {
        let dir = std::env::temp_dir().join(format!("temor-hosts-cache-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let included = dir.join("extra");
        let paths = ConfigPaths::beside(dir.join("config"), None);
        let cache = HostCache::default();
        assert!(cache.list(&paths).entries.is_empty());
        fs::write(&paths.config, "Include extra\n").unwrap();
        fs::write(&included, "Host one\n  HostName 10.0.0.1\n").unwrap();
        let later = |secs: u64, path: &Path| {
            fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(SystemTime::now() + Duration::from_secs(secs))
                .unwrap();
        };
        // Filesystems with coarse clocks need the times to move.
        later(5, &paths.config);
        later(5, &included);
        assert_eq!(cache.list(&paths).entries.len(), 1);
        fs::write(&included, "Host one\nHost two\n").unwrap();
        later(10, &included);
        assert_eq!(cache.list(&paths).entries.len(), 2);
        fs::remove_dir_all(&dir).unwrap();
    }
}
