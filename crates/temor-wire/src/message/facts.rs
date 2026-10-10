//! A session's folder as the chips read it: what git said and the version files found there.
//!
//! The helper only reads; the rules that turn this text into chips (the branch, the changes, the
//! versions) are temor's, so one set of rules serves a pane on this computer and one on a host.

use super::files::RemotePath;
use serde::{Deserialize, Serialize};

/// What the helper read about a session's folder. A part it could not read is left out; nothing
/// here is an error the person sees.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolderFacts {
    /// The folder.
    pub dir: RemotePath,
    /// What git said, when the folder is in a repository and git ran.
    #[serde(default)]
    pub git: Option<GitText>,
    /// The folder's `.venv`, when it has one.
    #[serde(default)]
    pub venv: Option<RemotePath>,
    /// The folder that holds `.git`, when the walk up from `dir` reached one.
    #[serde(default)]
    pub root: Option<RemotePath>,
    /// The version files in `dir` and each folder above it, up to `root` (or as far as the helper
    /// looks), nearest first.
    #[serde(default)]
    pub folders: Vec<FolderFiles>,
}

/// The text of the git runs, as git printed it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct GitText {
    /// `git rev-parse --abbrev-ref HEAD`, or `git symbolic-ref --short HEAD` in a repository with
    /// no commit.
    #[serde(default)]
    pub head: Option<String>,
    /// `git diff --shortstat HEAD`.
    #[serde(default)]
    pub shortstat: Option<String>,
    /// The `?? ` lines of `git status --porcelain=v1 --untracked-files=normal`, cut once there
    /// are more than the chip shows.
    #[serde(default)]
    pub untracked: Option<String>,
}

/// The version files found in one folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolderFiles {
    /// The folder.
    pub dir: RemotePath,
    /// The files that exist there and are small enough to read.
    pub files: Vec<FactFile>,
}

/// One version file's name and text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactFile {
    /// The file's name, such as `.nvmrc`.
    pub name: String,
    /// What it holds.
    pub text: String,
}
