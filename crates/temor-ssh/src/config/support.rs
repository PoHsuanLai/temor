//! Shared fixtures for the config tests: an in-memory config directory.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use super::include::{ConfigFiles, ConfigPaths};
use super::load_host_list;
use super::model::{HostEntry, HostList};

pub(super) const SSH_DIR: &str = "/fake/home/.ssh";

/// An in-memory config directory: file path to text.
pub(super) struct Fixture(BTreeMap<PathBuf, String>);

impl ConfigFiles for Fixture {
    fn read(&self, path: &Path) -> io::Result<String> {
        self.0
            .get(path)
            .cloned()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }

    fn list(&self, dir: &Path) -> io::Result<Vec<PathBuf>> {
        let mut found: Vec<PathBuf> = self
            .0
            .keys()
            .filter(|path| path.parent() == Some(dir))
            .cloned()
            .collect();
        found.sort();
        Ok(found)
    }
}

pub(super) fn load(files: &[(&str, &str)]) -> HostList {
    let map = files
        .iter()
        .map(|(name, text)| (PathBuf::from(SSH_DIR).join(name), (*text).to_owned()))
        .collect();
    let paths = ConfigPaths {
        config: PathBuf::from(SSH_DIR).join("config"),
        ssh_dir: PathBuf::from(SSH_DIR),
        home: Some(PathBuf::from("/fake/home")),
    };
    load_host_list(&Fixture(map), &paths)
}

pub(super) fn one(list: &HostList, alias: &str) -> HostEntry {
    list.entries
        .iter()
        .find(|entry| entry.alias.as_str() == alias)
        .cloned()
        .unwrap_or_else(|| panic!("no entry for {alias}: {:?}", list.entries))
}

pub(super) fn aliases(list: &HostList) -> Vec<&str> {
    list.entries.iter().map(|e| e.alias.as_str()).collect()
}
