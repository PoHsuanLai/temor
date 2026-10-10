//! The one-time clean-up of temor's old per-turn snapshots (PLAN P19). Agent checkpoints moved
//! to docket, which numbers and keeps its own; the `refs/temor/checkpoints/*` refs an earlier
//! temor made are not imported (they have no session), only deleted once they are older than
//! [`SNAPSHOT_KEEP`]. The rule is pure; the git calls are the edge.

use super::git::{Git, GitCommand, run_ok};
use crate::error::CoreError;

/// Where the old snapshots are.
pub const OLD_SNAPSHOT_REFS: &str = "refs/temor/checkpoints/";

/// How long an old snapshot is kept, in seconds: 14 days.
pub const SNAPSHOT_KEEP: u64 = 14 * 24 * 60 * 60;

/// The refs in `listing` to delete at `now` (seconds since the epoch). A line is
/// `<refname> <commit time in seconds>`, as `for-each-ref` prints it with
/// `--format=%(refname) %(committerdate:unix)`. A ref is stale when it is older than
/// [`SNAPSHOT_KEEP`]; anything outside [`OLD_SNAPSHOT_REFS`] or without a readable time is kept.
pub fn stale_snapshot_refs(listing: &str, now: u64) -> Vec<String> {
    listing
        .lines()
        .filter_map(|line| {
            let (name, time) = line.trim().split_once(' ')?;
            let time: u64 = time.trim().parse().ok()?;
            let old = now.saturating_sub(time) > SNAPSHOT_KEEP;
            (old && name.starts_with(OLD_SNAPSHOT_REFS) && name.len() > OLD_SNAPSHOT_REFS.len())
                .then(|| name.to_owned())
        })
        .collect()
}

/// Deletes the stale old snapshots of the repository at `repo`. Returns how many it deleted.
pub fn sweep_old_snapshots(git: &dyn Git, repo: &str, now: u64) -> Result<usize, CoreError> {
    let list = GitCommand::new(
        repo,
        &[
            "for-each-ref",
            "--format=%(refname) %(committerdate:unix)",
            OLD_SNAPSHOT_REFS,
        ],
    );
    let stale = stale_snapshot_refs(&run_ok(git, &list)?.text(), now);
    for name in &stale {
        run_ok(
            git,
            &GitCommand::new(repo, &["update-ref", "-d", name.as_str()]),
        )?;
    }
    Ok(stale.len())
}
