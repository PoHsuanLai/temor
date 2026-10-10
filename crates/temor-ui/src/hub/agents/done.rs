//! A finished job reaches the hub: each result goes to the part that asked for it.

use temor_core::worktree::overlap::OverlapIn;

use super::jobs::Done;
use super::state::Sheet;
use super::watching::Refresh;
use crate::hub::model::Hub;

impl Hub {
    /// Carries on from a finished job.
    pub fn agent_done(&mut self, done: Done) {
        match done {
            Done::PointsListed { pane, result } => self.points_listed(pane, result),
            Done::PointPlanned {
                pane,
                session,
                id,
                result,
            } => self.point_planned((pane, session, id), result),
            Done::PointRestored { pane, turn, result } => self.point_restored(pane, turn, result),
            Done::WatchOpened { pane, result } => self.watch_opened(pane, result),
            Done::WatchMarked { pane, result } => self.watch_marked(pane, result),
            Done::WatchEnded { pane, result } => self.watch_answered(pane, result, Refresh::Yes),
            Done::WatchClosed { pane, result } => self.watch_answered(pane, result, Refresh::No),
            Done::Swept { .. } => {}
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
        // Restore points are docket's; everything else here needs git.
        if let AgentPick::Restore(id) = pick {
            return self.ask_restore(tab, id);
        }
        if let Err(why) = self.agents.jobs.available() {
            self.notices.push(why.to_owned());
            return;
        }
        match pick {
            AgentPick::Restore(_) => {}
            AgentPick::RunInWorktree => self.ask_worktree(tab),
            AgentPick::MergeBack => self.ask_merge(tab),
            AgentPick::Details => self.agents.sheet = Some(Sheet::Details(tab)),
            AgentPick::Leftovers => self.ask_leftovers(tab),
        }
    }
}
