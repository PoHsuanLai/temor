//! Agent worktrees (D19, PLAN §8.2): temor's own worktrees on a new branch, set up from project
//! facts, with a deterministic early overlap check and a review-and-merge flow. The model and
//! the step functions are pure; every effect goes through the [`Git`] trait, whose process
//! implementation (temor-ui's `edge::git::ProcessGit`) takes the git binary's path from the caller.

mod create;
mod git;
mod life;
mod list;
mod merge;
mod model;
pub mod overlap;
mod setup;
mod snapshot;
mod sweep;
#[cfg(test)]
mod tests;
mod undo;

pub use create::{CreateParams, add_command, create, remove, resolve_commit, worktree_path};
pub use git::{Git, GitCommand, GitOutput, run_ok};
pub use life::{Life, LifeIn, LifeOut, worktree_life_step};
pub use list::{ListedWorktree, agent_worktrees, list_worktrees, parse_worktree_list};
pub use merge::{MergeOutcome, merge};
pub use model::{
    AgentWorktree, CloseChoice, MergeMethod, Oid, Placement, WorktreeName, placement_rows,
};
pub use setup::{INCLUDE_FILE, IncludeList, SetupPlan};
pub use snapshot::{Snapshot, snapshot, snapshot_index_path};
pub use sweep::{OLD_SNAPSHOT_REFS, SNAPSHOT_KEEP, stale_snapshot_refs, sweep_old_snapshots};
pub use undo::{next_undo, undo_ref, undo_ref_numbers};
