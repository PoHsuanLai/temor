//! Making and removing an agent worktree (D19): `git worktree add <data>/worktrees/<space>/<name>
//! -b agent/<name> <base>`, with the base commit recorded.

use super::git::{Git, GitCommand, run_ok};
use super::model::{AgentWorktree, Oid, WorktreeName, worktrees_root};
use crate::error::CoreError;
use crate::vocab::ids::TabId;

/// What a new worktree needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateParams {
    /// The agent's tab.
    pub tab: TabId,
    /// The main checkout.
    pub repo: String,
    /// temor's data folder (outside every repo).
    pub data_dir: String,
    /// The Space, as a folder name; cleaned the way a worktree name is.
    pub space: String,
    /// The worktree's name.
    pub name: WorktreeName,
    /// The branch or commit to cut from; `HEAD` when the person chose nothing.
    pub base_ref: String,
}

/// Where a worktree lives: `<data>/worktrees/<space>/<name>`. The Space is cleaned into one
/// path segment, so nothing can climb out.
pub fn worktree_path(
    data_dir: &str,
    space: &str,
    name: &WorktreeName,
) -> Result<String, CoreError> {
    let space = WorktreeName::from_task(space)?;
    Ok(format!(
        "{}/{}/{}",
        worktrees_root(data_dir),
        space.as_str(),
        name.as_str()
    ))
}

/// The commit `rev` names in `repo`; a bad ref is an error even though git exits 1 for it, as
/// it does for a conflict (spike: "refs validated with `rev-parse --verify` first").
pub fn resolve_commit(git: &dyn Git, repo: &str, rev: &str) -> Result<Oid, CoreError> {
    let target = format!("{rev}^{{commit}}");
    let cmd = GitCommand::new(repo, &["rev-parse", "--verify", "-q", target.as_str()]);
    let out = git.run(&cmd)?;
    match out.code {
        0 => Oid::parse(&out.text()),
        _ => Err(CoreError::not_a_commit(rev)),
    }
}

/// The git command that adds the worktree.
pub fn add_command(params: &CreateParams, path: &str, base: &Oid) -> GitCommand {
    let branch = params.name.branch();
    GitCommand::new(
        &params.repo,
        &[
            "worktree",
            "add",
            path,
            "-b",
            branch.as_str(),
            base.as_str(),
        ],
    )
}

/// Makes the worktree on a new branch and returns what to keep about it.
pub fn create(git: &dyn Git, params: &CreateParams) -> Result<AgentWorktree, CoreError> {
    let base = resolve_commit(git, &params.repo, &params.base_ref)?;
    let path = worktree_path(&params.data_dir, &params.space, &params.name)?;
    run_ok(git, &add_command(params, &path, &base))?;
    Ok(AgentWorktree {
        tab: params.tab,
        name: params.name.clone(),
        repo: params.repo.clone(),
        path,
        base_ref: params.base_ref.clone(),
        base,
    })
}

/// Removes the worktree folder and its branch (after a merge, or on Discard). The branch is
/// deleted with `-D` because a squash leaves it unmerged in git's eyes.
pub fn remove(git: &dyn Git, wt: &AgentWorktree) -> Result<(), CoreError> {
    run_ok(
        git,
        &GitCommand::new(
            &wt.repo,
            &["worktree", "remove", "--force", wt.path.as_str()],
        ),
    )?;
    let branch = wt.branch();
    run_ok(
        git,
        &GitCommand::new(&wt.repo, &["branch", "-D", branch.as_str()]),
    )?;
    Ok(())
}
