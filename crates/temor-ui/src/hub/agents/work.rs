//! What the agent jobs do, as plain functions over a [`Git`] runner and the filesystem: seed and
//! take checkpoints, restore one, make an agent worktree, snapshot and merge it, list leftovers.
//! They run on worker threads (never the render path) and know nothing of the window.

use std::fs;
use std::path::{Component, Path, PathBuf};

use temor_core::checkpoint::{RestorePlan, next_free, restore_files};
use temor_core::error::CoreError;
use temor_core::vocab::ids::{CheckpointRef, TabId};
use temor_core::vocab::text::RelPath;
use temor_core::worktree::overlap::parse_names;
use temor_core::worktree::{
    AgentWorktree, CreateParams, Git, GitCommand, INCLUDE_FILE, IncludeList, ListedWorktree,
    MergeMethod, MergeOutcome, Oid, Snapshot, WorktreeName, agent_worktrees, create,
    list_worktrees, merge, remove, resolve_commit, run_ok, snapshot,
};

use crate::error::UiError;

/// Where a run's checkpoints go and the number to start from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seeded {
    /// The repository's top folder, or `None` when the folder is not in a repository.
    pub root: Option<String>,
    pub next: CheckpointRef,
}

/// The top folder of the repository `cwd` is in.
pub fn toplevel(git: &dyn Git, cwd: &str) -> Option<String> {
    let out = git
        .run(&GitCommand::new(cwd, &["rev-parse", "--show-toplevel"]))
        .ok()?;
    let text = out.text();
    let root = text.trim_end();
    (out.code == 0 && !root.is_empty()).then(|| root.to_owned())
}

/// The repository of `cwd` and the first checkpoint number it has free.
pub fn seed(git: &dyn Git, cwd: &str) -> Seeded {
    let root = toplevel(git, cwd);
    let next = root
        .as_deref()
        .and_then(|root| next_free(git, root).ok())
        .unwrap_or(CheckpointRef(0));
    Seeded { root, next }
}

/// `rel` under `root`, or `None` when it is absolute or climbs out.
pub fn safe_join(root: &Path, rel: &str) -> Option<PathBuf> {
    let rel = Path::new(rel);
    let plain = rel
        .components()
        .all(|part| matches!(part, Component::Normal(_)));
    (plain && rel.components().next().is_some()).then(|| root.join(rel))
}

/// Puts the files back to checkpoint `at`, then removes the files made since. Returns how many
/// it removed. Only plain files and links are removed, never a folder.
pub fn restore(
    git: &dyn Git,
    root: &str,
    at: CheckpointRef,
    plan: &RestorePlan,
) -> Result<usize, UiError> {
    restore_files(git, root, at, plan)?;
    let mut removed = 0;
    for rel in &plan.delete {
        let Some(path) = safe_join(Path::new(root), rel.as_str()) else {
            continue;
        };
        let is_file = fs::symlink_metadata(&path).is_ok_and(|m| !m.is_dir());
        if is_file {
            fs::remove_file(&path).map_err(|source| UiError::AgentFile {
                path: rel.as_str().to_owned(),
                source,
            })?;
            removed += 1;
        }
    }
    Ok(removed)
}

/// What making a worktree needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MakeRequest {
    /// A folder inside the repository.
    pub cwd: String,
    /// temor's data folder.
    pub data_dir: String,
    /// The Space, as a folder name.
    pub space: String,
}

/// A worktree made, and how many ignored files were copied into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Made {
    pub worktree: AgentWorktree,
    pub copied: usize,
}

/// Makes an agent worktree for the repository `request.cwd` is in, on a new branch from where
/// the repository is now, and copies the files `.worktreeinclude` names.
pub fn make_worktree(git: &dyn Git, request: &MakeRequest) -> Result<Made, UiError> {
    let root = toplevel(git, &request.cwd).ok_or_else(|| {
        UiError::Git(CoreError::GitFailed {
            what: "rev-parse".to_owned(),
            stderr: "this folder is not in a git repository".to_owned(),
        })
    })?;
    let base_ref = current_branch(git, &root)?;
    let name = free_name(git, &root)?;
    let worktree = create(
        git,
        &CreateParams {
            tab: TabId(0),
            repo: root.clone(),
            data_dir: request.data_dir.clone(),
            space: request.space.clone(),
            name,
            base_ref,
        },
    )?;
    let copied = copy_included(git, &root, &worktree.path)?;
    Ok(Made { worktree, copied })
}

/// The branch checked out in `root`, or `HEAD` when detached.
fn current_branch(git: &dyn Git, root: &str) -> Result<String, UiError> {
    let out = run_ok(
        git,
        &GitCommand::new(root, &["rev-parse", "--abbrev-ref", "HEAD"]),
    )?;
    let text = out.text();
    Ok(match text.trim() {
        "" => "HEAD".to_owned(),
        branch => branch.to_owned(),
    })
}

/// A name from the repository's folder with the first number whose branch is free.
fn free_name(git: &dyn Git, root: &str) -> Result<WorktreeName, UiError> {
    let folder = root.rsplit('/').find(|p| !p.is_empty()).unwrap_or("work");
    let out = run_ok(
        git,
        &GitCommand::new(
            root,
            &[
                "for-each-ref",
                "--format=%(refname:short)",
                "refs/heads/agent/",
            ],
        ),
    )?;
    let taken = out.text();
    for n in 1..1000 {
        let name = WorktreeName::from_task(&format!("{folder} {n}"))?;
        if !taken.lines().any(|b| b == name.branch()) {
            return Ok(name);
        }
    }
    Err(CoreError::BadWorktreeName(folder.to_owned()).into())
}

/// Copies the ignored files `.worktreeinclude` lists from `root` into `target`.
fn copy_included(git: &dyn Git, root: &str, target: &str) -> Result<usize, UiError> {
    let Ok(text) = fs::read_to_string(Path::new(root).join(INCLUDE_FILE)) else {
        return Ok(0);
    };
    let list = IncludeList::parse(&text);
    let ignored = run_ok(
        git,
        &GitCommand::new(
            root,
            &[
                "ls-files",
                "--others",
                "--ignored",
                "--exclude-standard",
                "-z",
            ],
        ),
    )?;
    let ignored: Vec<RelPath> = parse_names(&ignored.stdout);
    let mut copied = 0;
    for rel in list.plan(&ignored, None).copy {
        let (Some(from), Some(to)) = (
            safe_join(Path::new(root), rel.as_str()),
            safe_join(Path::new(target), rel.as_str()),
        ) else {
            continue;
        };
        let plain = fs::symlink_metadata(&from).is_ok_and(|m| m.is_file());
        if !plain {
            continue;
        }
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent).map_err(|source| UiError::AgentFile {
                path: rel.as_str().to_owned(),
                source,
            })?;
        }
        fs::copy(&from, &to).map_err(|source| UiError::AgentFile {
            path: rel.as_str().to_owned(),
            source,
        })?;
        copied += 1;
    }
    Ok(copied)
}

/// The base branch's tip and a snapshot of the worktree, for the overlap check.
pub fn snapshot_tree(git: &dyn Git, wt: &AgentWorktree) -> Result<(Oid, Snapshot), UiError> {
    let tip = resolve_commit(git, &wt.repo, &wt.base_ref)?;
    Ok((tip, snapshot(git, &wt.path)?))
}

/// Commits whatever the agent left uncommitted, so the merge carries it.
fn commit_pending(git: &dyn Git, wt: &AgentWorktree) -> Result<(), CoreError> {
    let status = run_ok(git, &GitCommand::new(&wt.path, &["status", "--porcelain"]))?;
    if status.stdout.is_empty() {
        return Ok(());
    }
    run_ok(git, &GitCommand::new(&wt.path, &["add", "-A"]))?;
    let named = git
        .run(&GitCommand::new(&wt.path, &["config", "user.email"]))?
        .code
        == 0;
    let mut args = vec![];
    if !named {
        args.extend(["-c", "user.name=temor", "-c", "user.email=temor@localhost"]);
    }
    args.extend(["commit", "-q", "-m", "Work left by the agent"]);
    run_ok(git, &GitCommand::new(&wt.path, &args))?;
    Ok(())
}

/// Merges the worktree's branch into the checkout it was cut from, Squash. The base's old tip
/// is kept under a checkpoint ref so the merge can be undone.
pub fn merge_back(git: &dyn Git, wt: &AgentWorktree) -> Result<MergeOutcome, UiError> {
    commit_pending(git, wt)?;
    let undo = next_free(git, &wt.repo)?;
    let message = format!("Merge {}", wt.branch());
    Ok(merge(git, wt, MergeMethod::Squash, &message, undo)?)
}

/// Removes the worktree and its branch.
pub fn discard(git: &dyn Git, wt: &AgentWorktree) -> Result<(), UiError> {
    Ok(remove(git, wt)?)
}

/// temor's agent worktrees of `repo`, as git lists them (`data_dir` made real first).
pub fn find_leftovers(
    git: &dyn Git,
    repo: &str,
    data_dir: &Path,
) -> Result<Vec<ListedWorktree>, UiError> {
    let real = fs::canonicalize(data_dir).unwrap_or_else(|_| data_dir.to_path_buf());
    let listed = list_worktrees(git, repo)?;
    Ok(agent_worktrees(listed, &real.to_string_lossy()))
}

/// A listed worktree as the one to remove: only its repository, path and branch matter.
pub fn listed_as_worktree(repo: &str, listed: &ListedWorktree) -> Result<AgentWorktree, UiError> {
    let branch = listed.branch.as_deref().unwrap_or_default();
    let name = WorktreeName::from_task(branch.strip_prefix("agent/").unwrap_or(branch))?;
    Ok(AgentWorktree {
        tab: TabId(0),
        name,
        repo: repo.to_owned(),
        path: listed.path.clone(),
        base_ref: "HEAD".to_owned(),
        base: Oid::parse(&listed.head)?,
    })
}
