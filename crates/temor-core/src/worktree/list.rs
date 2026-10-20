//! Reading `git worktree list --porcelain`: which worktrees a repository has, and which of them
//! are temor's agent worktrees that no tab is using any more ("Leftover branches", D19).

use super::git::{Git, GitCommand, run_ok};
use super::model::{AGENT_BRANCH_PREFIX, worktrees_root};
use crate::error::CoreError;

/// One worktree git lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedWorktree {
    /// Where it is.
    pub path: String,
    /// The commit it has checked out (all zeros for a bare repository).
    pub head: String,
    /// Its branch, without `refs/heads/`; `None` when the head is detached.
    pub branch: Option<String>,
}

/// The worktrees in `text`, git's porcelain format: blocks of `key value` lines, one block per
/// worktree, separated by a blank line. A bare repository is skipped.
pub fn parse_worktree_list(text: &str) -> Vec<ListedWorktree> {
    let mut found = Vec::new();
    for block in text.split("\n\n") {
        let mut path = None;
        let mut head = String::new();
        let mut branch = None;
        let mut bare = false;
        for line in block.lines() {
            let (key, value) = line.split_once(' ').unwrap_or((line, ""));
            match key {
                "worktree" => path = Some(value.to_owned()),
                "HEAD" => head = value.to_owned(),
                "branch" => {
                    branch = Some(
                        value
                            .strip_prefix("refs/heads/")
                            .unwrap_or(value)
                            .to_owned(),
                    );
                }
                "bare" => bare = true,
                _ => {}
            }
        }
        if let (Some(path), false) = (path, bare) {
            found.push(ListedWorktree { path, head, branch });
        }
    }
    found
}

/// The worktrees of `repo`.
pub fn list_worktrees(git: &dyn Git, repo: &str) -> Result<Vec<ListedWorktree>, CoreError> {
    let cmd = GitCommand::new(repo, &["worktree", "list", "--porcelain"]);
    Ok(parse_worktree_list(&run_ok(git, &cmd)?.text()))
}

/// Of `listed`, the ones temor made: under `<data_dir>/worktrees/` and on an `agent/` branch.
/// `data_dir` should be the real path (git prints real paths).
pub fn agent_worktrees(listed: Vec<ListedWorktree>, data_dir: &str) -> Vec<ListedWorktree> {
    let prefix = format!("{}/", worktrees_root(data_dir));
    listed
        .into_iter()
        .filter(|w| {
            w.path.starts_with(&prefix)
                && w.branch
                    .as_deref()
                    .is_some_and(|b| b.starts_with(AGENT_BRANCH_PREFIX))
        })
        .collect()
}
