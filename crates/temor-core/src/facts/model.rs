//! The facts and their parts.

use std::fmt;

/// The checked-out git branch (`HEAD` when detached is not a branch and never appears here).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BranchName(pub String);

impl BranchName {
    /// The name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The name of a Python virtualenv: the last part of its folder.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EnvName(pub String);

impl EnvName {
    /// The name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A Node version as the project asks for it (`20`, `v18.19.0`, `>=18`), cut to be a chip.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeVersion(pub String);

impl NodeVersion {
    /// The version text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A Python, Ruby or Go version from `.tool-versions`, cut to be a chip.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ToolVersion(pub String);

impl ToolVersion {
    /// The version text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// How many files git has not been told about, counted up to [`MOST_UNTRACKED`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Untracked(pub u32);

/// The count stops here; more shows as `999+`.
pub const MOST_UNTRACKED: u32 = 999;

/// What `git diff --shortstat HEAD` says: files changed and lines added and removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct DiffStat {
    pub files: u32,
    pub added: u32,
    pub removed: u32,
}

impl DiffStat {
    /// Whether the work tree differs from `HEAD`.
    pub fn is_dirty(self) -> bool {
        self.files > 0
    }
}

/// What is known about a pane's folder. A fact that could not be read is `None`; none of them is
/// ever an error the person sees.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Facts {
    pub branch: Option<BranchName>,
    pub changes: Option<DiffStat>,
    pub venv: Option<EnvName>,
    pub node: Option<NodeVersion>,
    /// Files git does not track (and does not ignore); `None` when it could not be counted.
    pub untracked: Option<Untracked>,
    pub python: Option<ToolVersion>,
    pub ruby: Option<ToolVersion>,
    pub go: Option<ToolVersion>,
}

impl Facts {
    /// Nothing is known, so nothing is shown.
    pub fn is_empty(&self) -> bool {
        self.branch.is_none()
            && self.changes.is_none()
            && self.venv.is_none()
            && self.node.is_none()
            && self.untracked.is_none()
            && self.python.is_none()
            && self.ruby.is_none()
            && self.go.is_none()
    }

    /// Whether the folder has uncommitted changes.
    pub fn dirty(&self) -> bool {
        self.changes.is_some_and(DiffStat::is_dirty)
    }

    /// The short line for the tab's sidebar row: the branch, then a dot when there are changes.
    /// `None` outside a repository.
    pub fn summary(&self) -> Option<String> {
        let branch = self.branch.as_ref()?;
        Some(match self.dirty() {
            true => format!("{branch} \u{2022}"),
            false => branch.to_string(),
        })
    }
}

impl fmt::Display for BranchName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
