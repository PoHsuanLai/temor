//! A finished job reaches the hub: each result goes to the part that asked for it.

use temor_core::worktree::overlap::OverlapIn;

use super::jobs::Done;
use super::state::Sheet;
use crate::hub::model::Hub;

impl Hub {
    /// Carries on from a finished job.
    pub fn agent_done(&mut self, done: Done) {
        match done {
            Done::Seeded { pane, seeded } => self.checkpoint_seeded(pane, &seeded),
            Done::Taken { pane, at, result } => self.checkpoint_taken(pane, at, result),
            Done::Planned {
                pane,
                block,
                at,
                root,
                plan,
            } => self.checkpoint_planned((pane, block, at), root, plan),
            Done::Restored {
                pane,
                block,
                result,
            } => self.checkpoint_restored(pane, block, result),
            Done::Made { result } => self.worktree_made(result),
            Done::Snapshotted { tab, result } => self.tree_snapshotted(tab, result),
            Done::Pair(key, Ok(probe), _) => self.probe_done(OverlapIn::Pair(key, probe)),
            Done::Base(key, Ok(probe), _) => self.probe_done(OverlapIn::Base(key, probe)),
            Done::Pair(_, Err(_), job) | Done::Base(_, Err(_), job) => {
                self.probe_done(OverlapIn::Failed(job));
            }
            Done::Merged { tab, result } => self.merge_done(tab, result),
            Done::Removed { path, result } => self.removed(&path, result),
            Done::Listed { repo, result } => self.leftovers_listed(repo, result),
        }
    }

    fn removed(&mut self, path: &str, result: Result<(), crate::error::UiError>) {
        if let Err(err) = result {
            self.notices
                .push(format!("A worktree could not be removed: {err}"));
        }
        self.agents.trees.leftovers.retain(|w| w.path != path);
        let empty = self.agents.trees.leftovers.is_empty();
        if empty && matches!(self.agents.sheet, Some(Sheet::Leftovers { .. })) {
            self.agents.sheet = None;
        }
    }

    /// The person closed the sheet.
    pub fn close_agent_sheet(&mut self) {
        self.agents.sheet = None;
    }

    /// The person chose something in a tab's menu.
    pub fn agent_pick(&mut self, tab: temor_core::vocab::ids::TabId, pick: super::menu::AgentPick) {
        use super::menu::AgentPick;
        if let Err(why) = self.agents.jobs.available() {
            self.notices.push(why.to_owned());
            return;
        }
        match pick {
            AgentPick::Restore(block) => self.ask_restore(tab, block),
            AgentPick::RunInWorktree => self.ask_worktree(tab),
            AgentPick::MergeBack => self.ask_merge(tab),
            AgentPick::Details => self.agents.sheet = Some(Sheet::Details(tab)),
            AgentPick::Leftovers => self.ask_leftovers(tab),
        }
    }
}
