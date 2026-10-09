//! A snapshot of a worktree's state, uncommitted and untracked work included, made through a
//! private index so the agent's own index and files are never touched (spike: "the snapshot
//! step"). The commit has the worktree's HEAD as its parent; its tree id is the cache key.

use super::git::{Git, GitCommand, run_ok};
use super::model::Oid;
use crate::error::CoreError;

/// A worktree's state at one moment.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Snapshot {
    /// The snapshot commit (parent: the worktree's HEAD).
    pub commit: Oid,
    /// Its tree; equal trees mean equal files.
    pub tree: Oid,
}

/// The private index's path for a worktree: a fixed file inside its git directory, so nothing
/// needs cleaning up. Two snapshots of one worktree must not run at once; the runner takes them
/// one at a time per worktree.
pub fn snapshot_index_path(git: &dyn Git, worktree: &str) -> Result<String, CoreError> {
    let cmd = GitCommand::new(
        worktree,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            "temor-snapshot-index",
        ],
    );
    Ok(run_ok(git, &cmd)?.text().trim_end().to_owned())
}

/// Snapshots `worktree`: `read-tree HEAD`, `add -A`, `write-tree`, `commit-tree -p HEAD`, all
/// against the private index. Ignored files stay out, deletions and untracked files count.
pub fn snapshot(git: &dyn Git, worktree: &str) -> Result<Snapshot, CoreError> {
    let index = snapshot_index_path(git, worktree)?;
    let on_index =
        |args: &[&str]| GitCommand::new(worktree, args).with_env("GIT_INDEX_FILE", &index);
    run_ok(git, &on_index(&["read-tree", "HEAD"]))?;
    run_ok(git, &on_index(&["add", "-A"]))?;
    let tree = Oid::parse(&run_ok(git, &on_index(&["write-tree"]))?.text())?;
    let commit = run_ok(
        git,
        &GitCommand::new(
            worktree,
            &[
                "-c",
                "user.name=temor",
                "-c",
                "user.email=temor@localhost",
                "commit-tree",
                tree.as_str(),
                "-p",
                "HEAD",
                "-m",
                "temor snapshot",
            ],
        ),
    )?;
    Ok(Snapshot {
        commit: Oid::parse(&commit.text())?,
        tree,
    })
}
