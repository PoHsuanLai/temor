//! The hub's side of the leftover branches (D19): agent worktrees no tab uses, listed from git so
//! they can be opened again or removed.

use temor_core::vocab::ids::TabId;
use temor_core::workspace::WorkspaceIn;
use temor_core::worktree::ListedWorktree;

use super::checkpoints::cwd_of;
use super::jobs::Done;
use super::state::Sheet;
use super::work;
use crate::error::UiError;
use crate::hub::model::Hub;

impl Hub {
    /// "Leftover branches": the repository of the pane in front is asked what worktrees it has.
    pub(super) fn ask_leftovers(&mut self, tab: TabId) {
        let Some(cwd) = self.ws.focus_of(tab).and_then(|pane| self.pane_cwd(pane)) else {
            return;
        };
        let data_dir = self.agents.data_dir.clone();
        self.agents.jobs.run(move |git| {
            let repo = work::toplevel(git, &cwd).unwrap_or(cwd);
            let result = work::find_leftovers(git, &repo, &data_dir);
            Done::Listed { repo, result }
        });
    }

    pub(super) fn leftovers_listed(
        &mut self,
        repo: String,
        result: Result<Vec<ListedWorktree>, UiError>,
    ) {
        match result {
            Ok(listed) => {
                let in_use = |w: &ListedWorktree| self.worktree_in_use(&w.path);
                let free: Vec<ListedWorktree> = listed.into_iter().filter(|w| !in_use(w)).collect();
                match free.is_empty() {
                    true => self
                        .notices
                        .push("There are no leftover branches".to_owned()),
                    false => {
                        self.agents.trees.leftovers = free;
                        self.agents.sheet = Some(Sheet::Leftovers { repo });
                    }
                }
            }
            Err(err) => self
                .notices
                .push(format!("The branches could not be listed: {err}")),
        }
    }

    /// Whether a tab works in the worktree at `path`.
    fn worktree_in_use(&self, path: &str) -> bool {
        let inside = |cwd: &str| cwd == path || cwd.starts_with(&format!("{path}/"));
        self.ws.cwds.values().any(|c| inside(c.as_str()))
            || self
                .agents
                .trees
                .by_tab
                .values()
                .any(|t| t.worktree.path == path)
    }

    /// A leftover opens in a tab of its own.
    pub fn open_leftover(&mut self, path: &str) {
        self.act(WorkspaceIn::NewTab {
            cwd: Some(cwd_of(path)),
        });
        self.agents.sheet = None;
    }

    /// A leftover and its branch are removed.
    pub fn discard_leftover(&mut self, path: &str) {
        let Some(Sheet::Leftovers { repo }) = self.agents.sheet.clone() else {
            return;
        };
        let Some(listed) = self
            .agents
            .trees
            .leftovers
            .iter()
            .find(|w| w.path == path)
            .cloned()
        else {
            return;
        };
        self.agents.trees.leftovers.retain(|w| w.path != path);
        let path = path.to_owned();
        self.agents.jobs.run(move |git| Done::Removed {
            path,
            result: work::listed_as_worktree(&repo, &listed).and_then(|wt| work::discard(git, &wt)),
        });
    }
}
