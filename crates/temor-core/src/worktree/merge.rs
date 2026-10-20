//! Review and merge (D19): Squash (default), Merge, Rebase onto base, Open pull request. temor
//! runs git itself. A conflict is never resolved silently: the outcome lists the files and the
//! caller opens a shell split. Before any merge a hidden ref keeps the base's old tip so the
//! merge can be undone (`refs/temor/undo/<n>`, see `undo`).

use super::git::{Git, GitCommand, run_ok};
use super::model::{AgentWorktree, MergeMethod};
use super::overlap::parse_names;
use super::undo::undo_ref;
use crate::error::CoreError;
use crate::vocab::ids::UndoRef;
use crate::vocab::text::RelPath;

/// How a merge ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeOutcome {
    /// The agent's work is on the base. Remove the worktree and its branch.
    Merged,
    /// Git stopped on these files. Nothing is resolved; open a shell where they are listed.
    Conflicted {
        /// Where the merge is stuck: the folder to open the shell in.
        dir: String,
        /// The conflicted files.
        files: Vec<RelPath>,
    },
    /// Nothing was merged here: hand the branch to the host's pull request tool.
    PullRequest {
        /// The branch to open the request from.
        branch: String,
    },
}

/// Merges `wt` into the base checked out in its main checkout. `undo` is the number of the
/// undo ref that keeps the old tip. `message` is the commit message (Squash, Merge).
pub fn merge(
    git: &dyn Git,
    wt: &AgentWorktree,
    method: MergeMethod,
    message: &str,
    undo: UndoRef,
) -> Result<MergeOutcome, CoreError> {
    let branch = wt.branch();
    if method == MergeMethod::OpenPullRequest {
        return Ok(MergeOutcome::PullRequest { branch });
    }
    let undo_name = undo_ref(undo);
    run_ok(
        git,
        &GitCommand::new(&wt.repo, &["update-ref", undo_name.as_str(), "HEAD"]),
    )?;
    match method {
        MergeMethod::Squash => {
            let squash = GitCommand::new(&wt.repo, &["merge", "--squash", branch.as_str()]);
            let out = git.run(&squash)?;
            if out.code != 0 {
                return conflicted(git, &wt.repo, &squash, out.stderr);
            }
            // Nothing to commit (an empty squash) exits 1 and is still a finished merge.
            let commit = GitCommand::new(&wt.repo, &["commit", "-m", message]);
            git.run(&commit)?;
            Ok(MergeOutcome::Merged)
        }
        MergeMethod::Merge => {
            let cmd = GitCommand::new(
                &wt.repo,
                &["merge", "--no-ff", "-m", message, branch.as_str()],
            );
            let out = git.run(&cmd)?;
            match out.code {
                0 => Ok(MergeOutcome::Merged),
                _ => conflicted(git, &wt.repo, &cmd, out.stderr),
            }
        }
        MergeMethod::RebaseOntoBase => {
            let rebase = GitCommand::new(&wt.path, &["rebase", wt.base_ref.as_str()]);
            let out = git.run(&rebase)?;
            if out.code != 0 {
                return conflicted(git, &wt.path, &rebase, out.stderr);
            }
            run_ok(
                git,
                &GitCommand::new(&wt.repo, &["merge", "--ff-only", branch.as_str()]),
            )?;
            Ok(MergeOutcome::Merged)
        }
        MergeMethod::OpenPullRequest => Ok(MergeOutcome::PullRequest { branch }),
    }
}

/// Reads the unmerged files in `dir` after `cmd` stopped; no unmerged files means git failed
/// for another reason, which is an error rather than a conflict.
fn conflicted(
    git: &dyn Git,
    dir: &str,
    cmd: &GitCommand,
    stderr: String,
) -> Result<MergeOutcome, CoreError> {
    let unmerged = GitCommand::new(dir, &["diff", "--name-only", "--diff-filter=U", "-z"]);
    let files = parse_names(&run_ok(git, &unmerged)?.stdout);
    match files.is_empty() {
        true => Err(CoreError::git_failed(cmd.what(), &stderr)),
        false => Ok(MergeOutcome::Conflicted {
            dir: dir.to_owned(),
            files,
        }),
    }
}
