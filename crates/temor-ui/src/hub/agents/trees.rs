//! The agent worktrees of the window (D19): which tab has which, when its work is next looked at
//! for overlap, and the overlap check's memory. Pure bookkeeping over `overlap_step`; the git
//! work is `work.rs` on worker threads.

use std::collections::BTreeMap;

use temor_core::agent::radar::Millis;
use temor_core::vocab::ids::TabId;
use temor_core::worktree::overlap::{
    AgentOverlap, OverlapIn, OverlapOut, OverlapState, ProbeJob, overlap_for, overlap_step,
};
use temor_core::worktree::{AgentWorktree, Life, ListedWorktree, Oid, Snapshot};

/// How long after a command finishes the worktree is looked at, so a burst of commands costs one
/// look.
const SETTLE_MS: u64 = 2_000;

/// Where a worktree's next look stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    /// Nothing changed since the last look.
    Settled,
    /// A command finished at this time.
    Wanted(Millis),
    /// A snapshot is being made.
    Running,
}

/// One tab's worktree.
#[derive(Debug, Clone)]
pub struct Tree {
    pub worktree: AgentWorktree,
    pub life: Life,
    pub look: Look,
}

/// Every agent worktree of the window.
#[derive(Debug, Default)]
pub struct Trees {
    pub by_tab: BTreeMap<TabId, Tree>,
    overlap: OverlapState,
    /// The repository's agent worktrees no tab uses, as last listed.
    pub leftovers: Vec<ListedWorktree>,
}

impl Trees {
    /// A worktree now belongs to `tab`; its first look is due at once.
    pub fn adopt(&mut self, tab: TabId, mut worktree: AgentWorktree, now: Millis) {
        worktree.tab = tab;
        self.by_tab.insert(
            tab,
            Tree {
                worktree,
                life: Life::default(),
                look: Look::Wanted(Millis(now.0.saturating_sub(SETTLE_MS))),
            },
        );
    }

    /// The tab's worktree is gone (merged, discarded) or its tab closed.
    pub fn gone(&mut self, tab: TabId) {
        self.by_tab.remove(&tab);
        // Nothing is asked when a tab goes.
        self.feed(OverlapIn::Gone(tab));
    }

    /// A command finished in the tab's worktree.
    pub fn command_done(&mut self, tab: TabId, now: Millis) {
        if let Some(tree) = self.by_tab.get_mut(&tab)
            && tree.look == Look::Settled
        {
            tree.look = Look::Wanted(now);
        }
    }

    /// The worktrees to snapshot now: a command finished and the settling time has passed.
    pub fn due(&mut self, now: Millis) -> Vec<AgentWorktree> {
        let mut due = Vec::new();
        for tree in self.by_tab.values_mut() {
            if let Look::Wanted(since) = tree.look
                && now.since(since) >= SETTLE_MS
            {
                tree.look = Look::Running;
                due.push(tree.worktree.clone());
            }
        }
        due
    }

    /// A snapshot came back. Returns the comparisons to run.
    pub fn snapshotted(&mut self, tab: TabId, tip: Oid, snap: Snapshot) -> Vec<ProbeJob> {
        if let Some(tree) = self.by_tab.get_mut(&tab) {
            tree.look = Look::Settled;
        }
        let mut jobs = Vec::new();
        for input in [OverlapIn::BaseTip(tip), OverlapIn::Snapshotted(tab, snap)] {
            jobs.extend(self.feed(input));
        }
        jobs
    }

    /// The snapshot failed; the worktree is looked at again after the next command.
    pub fn snapshot_failed(&mut self, tab: TabId) {
        if let Some(tree) = self.by_tab.get_mut(&tab) {
            tree.look = Look::Settled;
        }
    }

    /// Feeds the overlap machine and returns the comparisons it asks for.
    pub fn feed(&mut self, input: OverlapIn) -> Vec<ProbeJob> {
        let (state, outs) = overlap_step(std::mem::take(&mut self.overlap), input);
        self.overlap = state;
        outs.into_iter()
            .filter_map(|out| match out {
                OverlapOut::Run(job) => Some(job),
                OverlapOut::Updated(_) => None,
            })
            .collect()
    }

    /// What the tab shows about its worktree.
    pub fn overlap(&self, tab: TabId) -> AgentOverlap {
        overlap_for(&self.overlap, tab)
    }
}
