//! `checkpoint_step` (D9, §8.2): a git snapshot before each agent block's first write, so every
//! agent block offers "Undo to here" and the run offers "Undo all". Snapshots live in hidden
//! refs, never a branch, the index or the stash.

mod git;
mod model;
mod step;
#[cfg(test)]
mod tests;

pub use git::{
    IGNORED_NOT_COVERED, RestorePlan, existing, files_in, next_free, parse_restore, ref_name,
    restore_files, restore_plan, take,
};
pub use model::{CheckpointPhase, Checkpoints, Mark};
pub use step::{CheckpointIn, CheckpointOut, checkpoint_step};
