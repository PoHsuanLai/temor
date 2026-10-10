//! Clearing the snapshots an earlier temor made (PLAN P19): per-turn restore points moved to
//! docket, so `refs/temor/checkpoints/*` older than 14 days are deleted from the repositories
//! the window starts in. Nothing is imported. The rule is `temor_core::worktree::stale_snapshot_refs`.

use std::collections::BTreeSet;

use super::jobs::Done;
use super::work::sweep_snapshots;
use crate::hub::model::Hub;

impl Hub {
    /// At start: the folders the restored panes are in are looked at once, on a worker thread.
    pub(in crate::hub) fn sweep_old_snapshots(&mut self) {
        let folders: BTreeSet<String> = self
            .ws
            .cwds
            .values()
            .map(|cwd| cwd.as_str().to_owned())
            .collect();
        if folders.is_empty() {
            return;
        }
        let now = crate::edge::unix_now().0;
        self.agents.jobs.run(move |git| Done::Swept {
            removed: folders
                .iter()
                .map(|cwd| sweep_snapshots(git, cwd, now).unwrap_or(0))
                .sum(),
        });
    }
}
