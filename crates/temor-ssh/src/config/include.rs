//! Where the config lives and how `Include` paths are found. The filesystem is injected through
//! [`ConfigFiles`], so the parser never touches the real `~/.ssh`.

use std::io;
use std::path::{Path, PathBuf};

use super::glob::{glob_match, is_wild};

/// The reads the config parser needs. The app implements it over the real filesystem; tests
/// use an in-memory copy.
pub trait ConfigFiles {
    /// The file's text. A missing file is an error of kind `NotFound`, which is not reported.
    fn read(&self, path: &Path) -> io::Result<String>;
    /// The entries of a directory, sorted by name. A missing directory is an error of kind
    /// `NotFound`, which is treated as empty.
    fn list(&self, dir: &Path) -> io::Result<Vec<PathBuf>>;
}

/// Where the config is and what `Include` and `~` resolve against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigPaths {
    /// The config file itself, usually `~/.ssh/config`.
    pub config: PathBuf,
    /// The directory relative `Include` paths resolve against, usually `~/.ssh`.
    pub ssh_dir: PathBuf,
    /// The person's home directory, for `~` in `Include` and `IdentityFile`. `None` when it is
    /// unknown; `~` paths then resolve to nothing.
    pub home: Option<PathBuf>,
}

impl ConfigPaths {
    /// The paths for a config file at `config`, with includes relative to its directory.
    pub fn beside(config: PathBuf, home: Option<PathBuf>) -> Self {
        let ssh_dir = config.parent().map(Path::to_path_buf).unwrap_or_default();
        Self {
            config,
            ssh_dir,
            home,
        }
    }
}

/// The files one `Include` argument names, in order. Relative paths resolve against
/// `ssh_dir`, `~/` against the home, and a leading `/` is absolute. Wildcards in any segment
/// match names within that directory.
pub(super) fn expand_include(
    paths: &ConfigPaths,
    files: &dyn ConfigFiles,
    pattern: &str,
) -> Vec<PathBuf> {
    let (root, rest) = if let Some(rest) = pattern.strip_prefix("~/") {
        match &paths.home {
            Some(home) => (home.clone(), rest),
            None => return Vec::new(),
        }
    } else if pattern.starts_with('/') {
        (PathBuf::from("/"), pattern.trim_start_matches('/'))
    } else {
        (paths.ssh_dir.clone(), pattern)
    };
    let mut candidates = vec![root];
    for segment in rest.split('/').filter(|s| !s.is_empty()) {
        candidates = step(candidates, segment, files);
    }
    candidates
}

fn step(dirs: Vec<PathBuf>, segment: &str, files: &dyn ConfigFiles) -> Vec<PathBuf> {
    if !is_wild(segment) {
        return dirs.into_iter().map(|dir| dir.join(segment)).collect();
    }
    let mut found = Vec::new();
    for dir in &dirs {
        // A directory that cannot be listed simply matches nothing.
        let Ok(entries) = files.list(dir) else {
            continue;
        };
        found.extend(entries.into_iter().filter(|entry| {
            entry
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| glob_match(segment, name))
        }));
    }
    found
}
