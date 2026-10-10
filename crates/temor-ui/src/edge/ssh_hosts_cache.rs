//! The `~/.ssh/config` host list, read again only when the file looks changed. The command bar
//! asks for the hosts on every keystroke; parsing the file each time is wasted work.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use temor_ssh::{ConfigPaths, FsConfigFiles, HostList, load_host_list};

/// What says the config changed: the file's and its folder's modified times (the folder's moves
/// when an included file is added or removed). `None` for a file that is not there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Stamp {
    config: PathBuf,
    file: Option<SystemTime>,
    folder: Option<SystemTime>,
}

impl Stamp {
    pub(super) fn of(paths: &ConfigPaths) -> Stamp {
        Stamp {
            config: paths.config.clone(),
            file: modified(&paths.config),
            folder: modified(&paths.ssh_dir),
        }
    }
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
}

/// The last list read and the stamp it was read at. Cloning shares the cache.
#[derive(Debug, Clone, Default)]
pub(super) struct HostCache(Arc<Mutex<Option<(Stamp, HostList)>>>);

impl HostCache {
    /// The list for `stamp`: the kept one when the stamp is the same, else `load`'s, kept.
    pub(super) fn get(&self, stamp: Stamp, load: impl FnOnce() -> HostList) -> HostList {
        let mut slot = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some((kept, list)) = slot.as_ref()
            && *kept == stamp
        {
            return list.clone();
        }
        let list = load();
        *slot = Some((stamp, list.clone()));
        list
    }

    /// The list for the config at `paths`, from disk only when it changed.
    pub(super) fn list(&self, paths: &ConfigPaths) -> HostList {
        self.get(Stamp::of(paths), || load_host_list(&FsConfigFiles, paths))
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::fs;
    use std::time::Duration;

    use super::*;

    fn stamp(file: u64) -> Stamp {
        Stamp {
            config: "/c".into(),
            file: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(file)),
            folder: None,
        }
    }

    #[test]
    fn the_file_is_parsed_again_only_when_its_stamp_changes() {
        let cache = HostCache::default();
        let loads = Cell::new(0);
        let load = || {
            loads.set(loads.get() + 1);
            HostList::default()
        };
        for (at, want) in [(1, 1), (1, 1), (2, 2), (2, 2), (1, 3)] {
            cache.get(stamp(at), load);
            assert_eq!(loads.get(), want, "stamp {at}");
        }
    }

    #[test]
    fn a_changed_file_on_disk_is_seen_and_a_missing_one_gives_an_empty_list() {
        let dir = std::env::temp_dir().join(format!("temor-hosts-cache-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let paths = ConfigPaths::beside(dir.join("config"), None);
        let cache = HostCache::default();
        assert!(cache.list(&paths).entries.is_empty());
        fs::write(&paths.config, "Host one\n  HostName 10.0.0.1\n").unwrap();
        // Filesystems with coarse clocks need the stamp to move; the folder's changes too.
        let later = SystemTime::now() + Duration::from_secs(5);
        fs::File::options()
            .write(true)
            .open(&paths.config)
            .unwrap()
            .set_modified(later)
            .unwrap();
        assert_eq!(cache.list(&paths).entries.len(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }
}
