//! `worktree_life_step`: an agent worktree from creation to removal, and what closing its tab
//! does. Worktrees persist with their tab; closing asks Keep, Merge or Discard; kept ones with
//! no tab are listed as leftover branches.

use super::merge::MergeOutcome;
use super::model::{CloseChoice, MergeMethod};
use crate::vocab::text::RelPath;

/// Where a worktree stands.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Life {
    /// In use by its tab.
    #[default]
    Open,
    /// A merge is running.
    Merging(MergeMethod),
    /// Git stopped on conflicts; the person has a shell there.
    Conflicted(Vec<RelPath>),
    /// Merged and being removed.
    Merged,
    /// Removed without merging.
    Discarded,
    /// Kept with no tab; listed under "Leftover branches".
    Leftover,
}

/// What moves a worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifeIn {
    /// The person pressed Squash, Merge, Rebase onto base or Open pull request.
    Merge(MergeMethod),
    /// A merge ended.
    Finished(MergeOutcome),
    /// The tab was closed and the person chose.
    CloseTab(CloseChoice),
    /// The person asked to remove a leftover.
    DiscardLeftover,
}

/// What to carry out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifeOut {
    /// Run `merge` with this method.
    RunMerge(MergeMethod),
    /// Remove the worktree and its branch.
    Remove,
    /// Open a shell split where the conflicted files are.
    OpenConflictShell(String),
    /// Hand the branch to the host's pull request tool.
    OpenPullRequest(String),
}

/// The worktree after `input`. Nothing runs twice: a merge while one is running, or after the
/// worktree is gone, is ignored.
pub fn worktree_life_step(life: Life, input: LifeIn) -> (Life, Vec<LifeOut>) {
    use Life::{Conflicted, Discarded, Leftover, Merged, Merging, Open};
    match (life, input) {
        (Open | Conflicted(_) | Leftover, LifeIn::Merge(method)) => {
            (Merging(method), vec![LifeOut::RunMerge(method)])
        }
        (Open | Conflicted(_), LifeIn::CloseTab(CloseChoice::Merge)) => {
            let method = MergeMethod::default();
            (Merging(method), vec![LifeOut::RunMerge(method)])
        }
        (Open | Conflicted(_), LifeIn::CloseTab(CloseChoice::Keep)) => (Leftover, vec![]),
        (
            Open | Conflicted(_) | Leftover,
            LifeIn::CloseTab(CloseChoice::Discard) | LifeIn::DiscardLeftover,
        ) => (Discarded, vec![LifeOut::Remove]),
        (Merging(_), LifeIn::Finished(MergeOutcome::Merged)) => (Merged, vec![LifeOut::Remove]),
        (Merging(_), LifeIn::Finished(MergeOutcome::Conflicted { dir, files })) => {
            (Conflicted(files), vec![LifeOut::OpenConflictShell(dir)])
        }
        (Merging(_), LifeIn::Finished(MergeOutcome::PullRequest { branch })) => {
            (Open, vec![LifeOut::OpenPullRequest(branch)])
        }
        (life, _) => (life, vec![]),
    }
}
