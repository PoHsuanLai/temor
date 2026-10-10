//! Which folders the client may reach, and the rule that turns a path into one of them.

use std::path::{Component, Path, PathBuf};

use temor_wire::message::RemotePath;

use crate::error::Error;

/// A path found inside a granted folder: the folder to open and the names below it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// The granted folder.
    pub root: PathBuf,
    /// The names from that folder down to the path; empty for the folder itself.
    pub rel: Vec<String>,
}

/// The folders the client has been given: the ones the daemon was configured with, the working
/// folder of each session it opened and the workspace of each relayed call. Anything else is
/// refused before a file is touched.
#[derive(Debug, Clone, Default)]
pub struct Scope {
    roots: Vec<PathBuf>,
}

impl Scope {
    /// A scope of `roots`. A root that is not an absolute path is left out.
    pub fn new(roots: impl IntoIterator<Item = PathBuf>) -> Self {
        let mut scope = Self::default();
        for root in roots {
            // A root the daemon was configured with that is unusable simply grants nothing.
            let _ = scope.grant(&root);
        }
        scope
    }

    /// Adds a folder. It must be an absolute path without `..`.
    pub fn grant(&mut self, root: &Path) -> Result<(), Error> {
        let clean = clean(root).ok_or_else(|| Error::BadPath {
            path: root.display().to_string(),
        })?;
        if !self.roots.contains(&clean) {
            self.roots.push(clean);
        }
        Ok(())
    }

    /// Finds `path` inside the most specific granted folder that holds it.
    pub fn resolve(&self, path: &RemotePath) -> Result<Resolved, Error> {
        let clean = clean(Path::new(&path.0)).ok_or_else(|| Error::BadPath {
            path: path.0.clone(),
        })?;
        let root = self
            .roots
            .iter()
            .filter(|root| clean.starts_with(root))
            .max_by_key(|root| root.components().count())
            .ok_or_else(|| Error::Outside {
                path: path.0.clone(),
            })?;
        let rel: Vec<String> = clean
            .strip_prefix(root)
            .map(|below| {
                below
                    .components()
                    .filter_map(|part| part.as_os_str().to_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        Ok(Resolved {
            root: root.clone(),
            rel,
        })
    }
}

/// The path without `.` parts, or `None` if it is relative or climbs with `..`.
fn clean(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::RootDir => out.push("/"),
            Component::Normal(name) => out.push(name),
            Component::CurDir => {}
            Component::ParentDir | Component::Prefix(_) => return None,
        }
    }
    out.is_absolute().then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope() -> Scope {
        Scope::new(["/home/me", "/home/me/work", "/srv/data"].map(PathBuf::from))
    }

    fn resolved(path: &str) -> Result<(String, Vec<String>), String> {
        scope()
            .resolve(&RemotePath(path.to_string()))
            .map(|r| (r.root.display().to_string(), r.rel))
            .map_err(|error| match error {
                Error::Outside { .. } => "outside".to_string(),
                Error::BadPath { .. } => "bad".to_string(),
                other => other.to_string(),
            })
    }

    #[test]
    fn a_path_lands_in_its_most_specific_folder_or_is_refused() {
        let owned = |root: &str, rel: &[&str]| {
            Ok((
                root.to_string(),
                rel.iter().map(|s| s.to_string()).collect(),
            ))
        };
        type Owned = Result<(String, Vec<String>), String>;
        let cases: Vec<(&str, Owned)> = vec![
            (
                "/home/me/work/a/b.txt",
                owned("/home/me/work", &["a", "b.txt"]),
            ),
            ("/home/me/notes.txt", owned("/home/me", &["notes.txt"])),
            ("/home/me/work", owned("/home/me/work", &[])),
            ("/home/me/./work//x", owned("/home/me/work", &["x"])),
            ("/home/mex/notes", Err("outside".to_string())),
            ("/etc/passwd", Err("outside".to_string())),
            ("/home/me/../../etc/passwd", Err("bad".to_string())),
            ("relative/path", Err("bad".to_string())),
            ("", Err("bad".to_string())),
        ];
        for (path, want) in cases {
            assert_eq!(resolved(path), want, "{path}");
        }
    }

    #[test]
    fn a_relative_or_climbing_grant_is_refused() {
        let mut scope = Scope::default();
        assert!(scope.grant(Path::new("work")).is_err());
        assert!(scope.grant(Path::new("/a/../b")).is_err());
        assert!(scope.grant(Path::new("/a/b")).is_ok());
    }
}
