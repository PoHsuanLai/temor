//! The words of D19: where an agent works, what it is called, and how its work comes back.

use crate::error::CoreError;
use crate::vocab::ids::TabId;
use ds_core::word::Word;
use serde::{Deserialize, Serialize};

/// Where a new agent works. The first one offered is the default; nothing guesses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum Placement {
    /// In a worktree temor makes, on its own branch.
    #[word(label = "On a new branch")]
    NewBranch,
    /// In the folder the person has open.
    #[word(label = "In this checkout")]
    InCheckout,
}

/// The two rows of the "New agent" choice, in order. A Space setting may put the checkout first.
pub fn placement_rows(checkout_first: Placement) -> [Placement; 2] {
    match checkout_first {
        Placement::InCheckout => [Placement::InCheckout, Placement::NewBranch],
        Placement::NewBranch => [Placement::NewBranch, Placement::InCheckout],
    }
}

/// How an agent's branch comes back to the base. Squash is the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Word, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum MergeMethod {
    /// One commit on the base with all the agent's changes.
    #[default]
    #[word(label = "Squash")]
    Squash,
    /// A merge commit that keeps the agent's commits.
    #[word(label = "Merge")]
    Merge,
    /// The agent's commits replayed on top of the base, in the agent's worktree.
    #[word(label = "Rebase onto base")]
    RebaseOntoBase,
    /// Nothing merged here: the branch goes to the host's pull request tool.
    #[word(label = "Open pull request")]
    OpenPullRequest,
}

/// What closing an agent's tab does to its worktree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum CloseChoice {
    /// Leave it; it is listed in the Space as a leftover branch.
    Keep,
    /// Merge it (Squash), then remove it.
    Merge,
    /// Remove the worktree and the branch.
    Discard,
}

/// A commit or tree id as git prints it: 40 or 64 lowercase hex digits.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Oid(String);

impl Oid {
    /// `text` (a trailing newline allowed) as an id, or `NotACommit`.
    pub fn parse(text: &str) -> Result<Oid, CoreError> {
        let text = text.trim_end_matches(['\n', '\r']);
        let ok = matches!(text.len(), 40 | 64)
            && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        match ok {
            true => Ok(Oid(text.to_owned())),
            false => Err(CoreError::NotACommit(text.chars().take(80).collect())),
        }
    }

    /// The id text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A worktree's short name: lowercase letters, digits and single dashes, 1 to 40 characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorktreeName(String);

impl WorktreeName {
    /// A name from a task description or what the person typed: letters and digits kept
    /// (lowercased ASCII), every other run becomes one dash, ends trimmed, cut at 40.
    pub fn from_task(task: &str) -> Result<WorktreeName, CoreError> {
        let mut out = String::new();
        for c in task.chars() {
            match c.is_ascii_alphanumeric() {
                true => out.push(c.to_ascii_lowercase()),
                false if !out.is_empty() && !out.ends_with('-') => out.push('-'),
                false => {}
            }
            if out.len() >= 40 {
                break;
            }
        }
        let name = out.trim_end_matches('-');
        match name.is_empty() {
            true => Err(CoreError::BadWorktreeName(task.chars().take(60).collect())),
            false => Ok(WorktreeName(name.to_owned())),
        }
    }

    /// The name.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The branch the agent works on: `agent/<name>`.
    pub fn branch(&self) -> String {
        format!("agent/{}", self.0)
    }
}

/// One agent worktree temor made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentWorktree {
    /// The agent's tab.
    pub tab: TabId,
    /// Its name.
    pub name: WorktreeName,
    /// The main checkout it was cut from.
    pub repo: String,
    /// Where it lives, in temor's data folder.
    pub path: String,
    /// The branch the person chose as base (`main`), kept for merging back.
    pub base_ref: String,
    /// The base commit when the worktree was made.
    pub base: Oid,
}

impl AgentWorktree {
    /// Its branch.
    pub fn branch(&self) -> String {
        self.name.branch()
    }
}
