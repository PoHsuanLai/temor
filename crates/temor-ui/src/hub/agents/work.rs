//! What the agent jobs do, as plain functions over a [`Git`] runner and the filesystem: make an
//! agent worktree, snapshot and merge it, list leftovers, sweep old snapshot refs.
//! They run on worker threads (never the render path) and know nothing of the window.

use std::fs;
use std::path::{Component, Path, PathBuf};

use temor_core::error::CoreError;
use temor_core::vocab::ids::TabId;
use temor_core::vocab::text::RelPath;
use temor_core::worktree::overlap::parse_names;
use temor_core::worktree::{
    AgentWorktree, CreateParams, Git, GitCommand, INCLUDE_FILE, IncludeList, ListedWorktree,
    MergeMethod, MergeOutcome, Oid, Snapshot, WorktreeName, agent_worktrees, create,
    list_worktrees, merge, next_undo, remove, resolve_commit, run_ok, snapshot,
    sweep_old_snapshots,
};

use crate::error::UiError;

/// The top folder of the repository `cwd` is in.
pub fn toplevel(git: &dyn Git, cwd: &str) -> Option<String> {
    let out = git
        .run(&GitCommand::new(cwd, &["rev-parse", "--show-toplevel"]))
        .ok()?;
    let text = out.text();
    let root = text.trim_end();
    (out.code == 0 && !root.is_empty()).then(|| root.to_owned())
}

/// `rel` under `root`, or `None` when it is absolute or climbs out.
pub fn safe_join(root: &Path, rel: &str) -> Option<PathBuf> {
    let rel = Path::new(rel);
    let plain = rel
        .components()
        .all(|part| matches!(part, Component::Normal(_)));
    (plain && rel.components().next().is_some()).then(|| root.join(rel))
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
            // Made real, so the path recorded here is the one git lists later.
            data_dir: real(Path::new(&request.data_dir)),
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
/// is kept under `refs/temor/undo/<n>` so the merge can be undone.
pub fn merge_back(git: &dyn Git, wt: &AgentWorktree) -> Result<MergeOutcome, UiError> {
    commit_pending(git, wt)?;
    let undo = next_undo(git, &wt.repo)?;
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
    let listed = list_worktrees(git, repo)?;
    Ok(agent_worktrees(listed, &real(data_dir)))
}

/// Of `listed`, the worktrees no tab works in. `used` holds the tabs' folders as their shells
/// report them, which keep any symlink in the path while git lists the real path, so both are
/// compared made real (a data folder under a symlinked home, as Fedora Atomic's `/home` is).
pub fn unused_worktrees(listed: Vec<ListedWorktree>, used: &[String]) -> Vec<ListedWorktree> {
    let used: Vec<String> = used.iter().map(|path| real(Path::new(path))).collect();
    listed
        .into_iter()
        .filter(|w| {
            let path = real(Path::new(&w.path));
            !used
                .iter()
                .any(|cwd| *cwd == path || cwd.starts_with(&format!("{path}/")))
        })
        .collect()
}

/// `path` with every symlink resolved. The part not made yet (or gone) is kept as written, under
/// its deepest folder that is there, made real.
fn real(path: &Path) -> String {
    let mut rest = Vec::new();
    let mut there = path;
    let resolved = loop {
        if let Ok(found) = fs::canonicalize(there) {
            break found;
        }
        match (there.parent(), there.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name);
                there = parent;
            }
            _ => return path.to_string_lossy().into_owned(),
        }
    };
    let whole = rest.iter().rev().fold(resolved, |at, name| at.join(name));
    whole.to_string_lossy().into_owned()
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

/// Deletes the old per-turn snapshot refs (`refs/temor/checkpoints/*`) older than 14 days from
/// the repository `cwd` is in. Returns how many it deleted; a folder outside a repository has none.
pub fn sweep_snapshots(git: &dyn Git, cwd: &str, now: u64) -> Result<usize, UiError> {
    let Some(root) = toplevel(git, cwd) else {
        return Ok(0);
    };
    Ok(sweep_old_snapshots(git, &root, now)?)
}
