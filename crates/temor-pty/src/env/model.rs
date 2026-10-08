//! `ChildEnv`: the complete environment a child starts with.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};

/// Every variable the child gets, and nothing else: spawning clears the parent's environment
/// first. Keys are unique; setting one again replaces it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChildEnv {
    vars: BTreeMap<OsString, OsString>,
}

impl ChildEnv {
    /// An empty environment.
    pub fn new() -> Self {
        Self::default()
    }

    /// The value of `key`, if set.
    pub fn get(&self, key: impl AsRef<OsStr>) -> Option<&OsStr> {
        self.vars.get(key.as_ref()).map(OsString::as_os_str)
    }

    /// Sets `key` to `value`, replacing any earlier value.
    pub fn set(&mut self, key: impl Into<OsString>, value: impl Into<OsString>) {
        self.vars.insert(key.into(), value.into());
    }

    /// Removes `key`, returning its value.
    pub fn remove(&mut self, key: impl AsRef<OsStr>) -> Option<OsString> {
        self.vars.remove(key.as_ref())
    }

    /// The variables, sorted by name.
    pub fn iter(&self) -> impl Iterator<Item = (&OsStr, &OsStr)> {
        self.vars
            .iter()
            .map(|(key, value)| (key.as_os_str(), value.as_os_str()))
    }

    /// Every variable of `other` set over this one.
    pub fn overlay(mut self, other: &ChildEnv) -> Self {
        for (key, value) in other.iter() {
            self.set(key, value);
        }
        self
    }
}

impl<K: Into<OsString>, V: Into<OsString>> FromIterator<(K, V)> for ChildEnv {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        Self {
            vars: iter
                .into_iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
        }
    }
}
