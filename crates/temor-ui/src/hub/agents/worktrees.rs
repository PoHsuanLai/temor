//! The hub's side of agent worktrees (D19): "Run in a worktree", the overlap check after a
//! command finishes and merging back (Squash). temor creates the
//! worktree in its own data folder and opens a tab in it; the agent is just given a folder.

use temor_core::agent::radar::Millis;
use temor_core::vocab::ids::{PaneId, TabId};
use temor_core::workspace::WorkspaceIn;
use temor_core::worktree::overlap::{OverlapIn, ProbeJob, probe_base, probe_pair};
use temor_core::worktree::{
    AgentWorktree, Life, LifeIn, LifeOut, MergeMethod, MergeOutcome, worktree_life_step,
};

use super::checkpoints::cwd_of;
use super::jobs::Done;
use super::state::SPACE;
use super::work::{self, Made, MakeRequest};
use crate::error::UiError;
use crate::hub::model::Hub;

impl Hub {
    /// "Run in a worktree" on `tab`: the repository of its pane gets a worktree on a new branch.
    pub(super) fn ask_worktree(&mut self, tab: TabId) {
        let Some(cwd) = self.ws.focus_of(tab).and_then(|pane| self.pane_cwd(pane)) else {
            return;
        };
        let request = MakeRequest {
            cwd,
            data_dir: self.agents.data_dir.to_string_lossy().into_owned(),
            space: SPACE.to_owned(),
        };
        self.agents.jobs.run(move |git| Done::Made {
            result: work::make_worktree(git, &request),
        });
    }

    /// The worktree is made: a new tab opens in it and takes it over.
    pub(super) fn worktree_made(&mut self, result: Result<Made, UiError>) {
        let made = match result {
            Ok(made) => made,
            Err(err) => {
                self.notices
                    .push(format!("A worktree could not be made: {err}"));
                return;
            }
        };
        self.act(WorkspaceIn::NewTab {
            cwd: Some(cwd_of(&made.worktree.path)),
        });
        let now = self.agents.now();
        if let Some(tab) = self.ws.front() {
            self.agents.trees.adopt(tab, made.worktree.clone(), now);
        }
        let copied = match made.copied {
            0 => String::new(),
            n => format!(", {n} ignored files copied"),
        };
        self.notices.push(format!(
            "Opened {} on its own branch{copied}",
            made.worktree.branch()
        ));
    }

    /// A command finished in `pane`: if its tab is an agent worktree, it is looked at soon.
    pub(in crate::hub) fn agents_command_done(&mut self, pane: PaneId) {
        let Some(tab) = self.ws.tab_of(pane) else {
            return;
        };
        let now = self.agents.now();
        self.agents.trees.command_done(tab, now);
    }

    /// Worktrees whose command finished a moment ago are snapshotted off the render path.
    pub(super) fn look_at_due_trees(&mut self, now: Millis) {
        for wt in self.agents.trees.due(now) {
            let tab = wt.tab;
            self.agents.jobs.run(move |git| Done::Snapshotted {
                tab,
                result: work::snapshot_tree(git, &wt),
            });
        }
    }

    pub(super) fn tree_snapshotted(
        &mut self,
        tab: TabId,
        result: Result<(temor_core::worktree::Oid, temor_core::worktree::Snapshot), UiError>,
    ) {
        let Some(repo) = self
            .agents
            .trees
            .by_tab
            .get(&tab)
            .map(|t| t.worktree.repo.clone())
        else {
            return;
        };
        match result {
            Ok((tip, snap)) => {
                for job in self.agents.trees.snapshotted(tab, tip, snap) {
                    self.run_probe(repo.clone(), job);
                }
            }
            Err(_) => self.agents.trees.snapshot_failed(tab),
        }
    }

    fn run_probe(&self, repo: String, job: ProbeJob) {
        let again = job.clone();
        self.agents.jobs.run(move |git| match job {
            ProbeJob::Pair { key, a, b } => Done::Pair(
                key,
                probe_pair(git, &repo, &a, &b).map_err(UiError::from),
                again,
            ),
            ProbeJob::Base { key, base, agent } => Done::Base(
                key,
                probe_base(git, &repo, &base, &agent).map_err(UiError::from),
                again,
            ),
        });
    }

    /// An overlap comparison came back (or failed, and may be asked for again).
    pub(super) fn probe_done(&mut self, input: OverlapIn) {
        let repo = self
            .agents
            .trees
            .by_tab
            .values()
            .next()
            .map(|t| t.worktree.repo.clone());
        for job in self.agents.trees.feed(input) {
            if let Some(repo) = &repo {
                self.run_probe(repo.clone(), job);
            }
        }
    }

    /// Worktrees whose tab was closed stay on disk, listed as leftovers; the window stops
    /// watching them.
    pub(super) fn forget_closed_trees(&mut self) {
        let open: Vec<TabId> = self.ws.tabs.place.tabs.iter().map(|t| t.id).collect();
        let closed: Vec<TabId> = self
            .agents
            .trees
            .by_tab
            .keys()
            .filter(|tab| !open.contains(tab))
            .copied()
            .collect();
        for tab in closed {
            self.agents.trees.gone(tab);
        }
    }

    /// "Merge back": Squash, in the checkout the worktree was cut from.
    pub(super) fn ask_merge(&mut self, tab: TabId) {
        let Some(tree) = self.agents.trees.by_tab.get_mut(&tab) else {
            return;
        };
        let (life, outs) = worktree_life_step(
            std::mem::take(&mut tree.life),
            LifeIn::Merge(MergeMethod::Squash),
        );
        tree.life = life;
        let wt = tree.worktree.clone();
        for out in outs {
            if let LifeOut::RunMerge(_) = out {
                let wt = wt.clone();
                self.agents.jobs.run(move |git| Done::Merged {
                    tab,
                    result: work::merge_back(git, &wt),
                });
            }
        }
    }

    pub(super) fn merge_done(&mut self, tab: TabId, result: Result<MergeOutcome, UiError>) {
        let Some(tree) = self.agents.trees.by_tab.get_mut(&tab) else {
            return;
        };
        let branch = tree.worktree.branch();
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(err) => {
                tree.life = Life::Open;
                self.notices
                    .push(format!("{branch} could not be merged: {err}"));
                return;
            }
        };
        let (life, outs) =
            worktree_life_step(std::mem::take(&mut tree.life), LifeIn::Finished(outcome));
        tree.life = life;
        let wt = tree.worktree.clone();
        for out in outs {
            match out {
                LifeOut::Remove => self.remove_worktree(tab, wt.clone()),
                LifeOut::OpenConflictShell(dir) => {
                    self.notices.push(format!(
                        "{branch} stopped on conflicts. Nothing is resolved for you; a shell is open in {dir}"
                    ));
                    self.act(WorkspaceIn::NewTab {
                        cwd: Some(cwd_of(&dir)),
                    });
                }
                LifeOut::RunMerge(_) | LifeOut::OpenPullRequest(_) => {}
            }
        }
    }

    fn remove_worktree(&mut self, tab: TabId, wt: AgentWorktree) {
        self.agents.trees.gone(tab);
        self.notices.push(format!(
            "{} is merged into your checkout; the old state is kept so it can be undone",
            wt.branch()
        ));
        let path = wt.path.clone();
        self.agents.jobs.run(move |git| Done::Removed {
            path,
            result: work::discard(git, &wt),
        });
    }
}
