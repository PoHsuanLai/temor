//! The git side of checkpoints (D9, §8.2): `refs/temor/checkpoints/<n>` holds a snapshot of
//! tracked and untracked files, made through a private index so the person's index, stash and
//! branches are never touched. Ignored files are not covered, and the UI says so.

use crate::error::CoreError;
use crate::vocab::ids::CheckpointRef;
use crate::vocab::text::RelPath;
use crate::worktree::overlap::parse_names;
use crate::worktree::{Git, GitCommand, Snapshot, run_ok, snapshot, snapshot_index_path};

/// The note shown with every checkpoint: what it leaves out.
pub const IGNORED_NOT_COVERED: &str = "Ignored files are not covered";

const PREFIX: &str = "refs/temor/checkpoints/";
/// Paths per `checkout-index` call, to stay under argument limits.
const CHUNK: usize = 200;

/// The hidden ref for checkpoint `n`.
pub fn ref_name(n: CheckpointRef) -> String {
    format!("{PREFIX}{}", n.0)
}

/// Snapshots `root` and stores it as checkpoint `n`. Call before the agent's first write.
pub fn take(git: &dyn Git, root: &str, n: CheckpointRef) -> Result<Snapshot, CoreError> {
    let snap = snapshot(git, root)?;
    let name = ref_name(n);
    run_ok(
        git,
        &GitCommand::new(root, &["update-ref", name.as_str(), snap.commit.as_str()]),
    )?;
    Ok(snap)
}

/// The checkpoint numbers that exist, ascending.
pub fn existing(git: &dyn Git, root: &str) -> Result<Vec<CheckpointRef>, CoreError> {
    let cmd = GitCommand::new(root, &["for-each-ref", "--format=%(refname)", PREFIX]);
    let mut found: Vec<CheckpointRef> = run_ok(git, &cmd)?
        .text()
        .lines()
        .filter_map(|l| l.strip_prefix(PREFIX)?.parse().ok().map(CheckpointRef))
        .collect();
    found.sort();
    Ok(found)
}

/// The number a new run should start from, so it never overwrites an older checkpoint.
pub fn next_free(git: &dyn Git, root: &str) -> Result<CheckpointRef, CoreError> {
    Ok(existing(git, root)?
        .last()
        .map_or(CheckpointRef(0), |n| CheckpointRef(n.0 + 1)))
}

/// What putting the files back to a checkpoint changes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RestorePlan {
    /// Files to write from the checkpoint (changed or deleted since).
    pub write: Vec<RelPath>,
    /// Files made since: the caller deletes them (the core touches no files).
    pub delete: Vec<RelPath>,
}

/// Reads `diff-tree --name-status -z` output (current state against the checkpoint): `A` is in
/// the checkpoint only, `D` is in the current state only, anything else changed.
pub fn parse_restore(stdout: &[u8]) -> RestorePlan {
    let mut plan = RestorePlan::default();
    let mut fields = stdout.split(|b| *b == 0).filter(|f| !f.is_empty());
    while let (Some(status), Some(path)) = (fields.next(), fields.next()) {
        let path = RelPath(String::from_utf8_lossy(path).into_owned());
        match status.first() {
            Some(b'D') => plan.delete.push(path),
            Some(_) => plan.write.push(path),
            None => {}
        }
    }
    plan
}

/// What restoring checkpoint `n` would change in `root` right now.
pub fn restore_plan(git: &dyn Git, root: &str, n: CheckpointRef) -> Result<RestorePlan, CoreError> {
    let now = snapshot(git, root)?;
    let name = ref_name(n);
    let cmd = GitCommand::new(
        root,
        &[
            "diff-tree",
            "-r",
            "--no-renames",
            "--name-status",
            "-z",
            now.tree.as_str(),
            name.as_str(),
        ],
    );
    Ok(parse_restore(&run_ok(git, &cmd)?.stdout))
}

/// Writes the plan's files back from checkpoint `n` through a private index, then drops the
/// checkpoints after `n`. Deleting `plan.delete` is the caller's.
pub fn restore_files(
    git: &dyn Git,
    root: &str,
    n: CheckpointRef,
    plan: &RestorePlan,
) -> Result<(), CoreError> {
    let index = snapshot_index_path(git, root)?;
    let name = ref_name(n);
    let on_index = |args: &[&str]| GitCommand::new(root, args).with_env("GIT_INDEX_FILE", &index);
    run_ok(git, &on_index(&["read-tree", name.as_str()]))?;
    for chunk in plan.write.chunks(CHUNK) {
        let mut args = vec!["checkout-index", "-f", "--"];
        args.extend(chunk.iter().map(RelPath::as_str));
        run_ok(git, &on_index(&args))?;
    }
    for later in existing(git, root)?.into_iter().filter(|m| m.0 > n.0) {
        let name = ref_name(later);
        run_ok(
            git,
            &GitCommand::new(root, &["update-ref", "-d", name.as_str()]),
        )?;
    }
    Ok(())
}

/// Paths from a `-z` name list, for callers that need the checkpoint's file list.
pub fn files_in(git: &dyn Git, root: &str, n: CheckpointRef) -> Result<Vec<RelPath>, CoreError> {
    let name = ref_name(n);
    let cmd = GitCommand::new(root, &["ls-tree", "-r", "--name-only", "-z", name.as_str()]);
    Ok(parse_names(&run_ok(git, &cmd)?.stdout))
}
