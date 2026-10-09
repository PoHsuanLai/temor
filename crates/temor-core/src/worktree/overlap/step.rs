//! `overlap_step`: which comparisons to run when an agent's snapshot changes, and what the tab
//! shows once they are in. Results are cached by tree id, so only pairs whose trees changed are
//! recomputed. The runner (a worker thread the caller owns) runs each [`ProbeJob`] with
//! `probe_pair` or `probe_base` and feeds the answer back; running them is off the render path.

use std::collections::{BTreeMap, BTreeSet};

use super::model::{AgentOverlap, Divergence, Finding, Other};
use super::probe::{BaseProbe, PairProbe};
use crate::vocab::ids::TabId;
use crate::worktree::model::Oid;
use crate::worktree::snapshot::Snapshot;

/// Two agents' trees, smaller first, so one entry serves both directions.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PairKey(Oid, Oid);

impl PairKey {
    fn of(a: &Oid, b: &Oid) -> PairKey {
        match a <= b {
            true => PairKey(a.clone(), b.clone()),
            false => PairKey(b.clone(), a.clone()),
        }
    }
}

/// An agent's tree against a base tip.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BaseKey {
    tree: Oid,
    base: Oid,
}

impl BaseKey {
    #[cfg(test)]
    pub(super) fn for_test(snap: &Snapshot, base: &Oid) -> BaseKey {
        BaseKey {
            tree: snap.tree.clone(),
            base: base.clone(),
        }
    }
}

/// The overlap check's memory.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OverlapState {
    agents: BTreeMap<TabId, Snapshot>,
    base: Option<Oid>,
    pairs: BTreeMap<PairKey, PairProbe>,
    bases: BTreeMap<BaseKey, BaseProbe>,
    inflight_pairs: BTreeSet<PairKey>,
    inflight_bases: BTreeSet<BaseKey>,
}

/// What moves the check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlapIn {
    /// The base branch's tip commit (at start, and when it moves).
    BaseTip(Oid),
    /// An agent's worktree was snapshotted (after a command finished, debounced).
    Snapshotted(TabId, Snapshot),
    /// An agent's tab closed or its worktree was removed.
    Gone(TabId),
    /// A pair job finished.
    Pair(PairKey, PairProbe),
    /// A base job finished.
    Base(BaseKey, BaseProbe),
    /// A job failed (git refused); allow it to be asked for again.
    Failed(ProbeJob),
}

/// A comparison to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeJob {
    /// Two agents' snapshots.
    Pair {
        /// The cache key to answer with.
        key: PairKey,
        /// One snapshot commit.
        a: Oid,
        /// The other.
        b: Oid,
    },
    /// An agent's snapshot and the base tip.
    Base {
        /// The cache key to answer with.
        key: BaseKey,
        /// The base tip commit.
        base: Oid,
        /// The agent's snapshot commit.
        agent: Oid,
    },
}

/// What follows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlapOut {
    /// Run this comparison.
    Run(ProbeJob),
    /// This tab's findings changed; redraw it.
    Updated(TabId),
}

/// The check after `input`.
pub fn overlap_step(mut state: OverlapState, input: OverlapIn) -> (OverlapState, Vec<OverlapOut>) {
    let mut out = Vec::new();
    match input {
        OverlapIn::BaseTip(tip) if state.base.as_ref() == Some(&tip) => {}
        OverlapIn::BaseTip(tip) => {
            state.base = Some(tip);
            let tabs: Vec<TabId> = state.agents.keys().copied().collect();
            for tab in tabs {
                ask_base(&mut state, tab, &mut out);
            }
        }
        OverlapIn::Snapshotted(tab, snap) => {
            if state
                .agents
                .get(&tab)
                .is_some_and(|old| old.tree == snap.tree)
            {
                return (state, out);
            }
            state.agents.insert(tab, snap);
            let others: Vec<(TabId, Snapshot)> = state
                .agents
                .iter()
                .filter(|(t, _)| **t != tab)
                .map(|(t, s)| (*t, s.clone()))
                .collect();
            let mine = state.agents[&tab].clone();
            for (_, other) in others {
                let key = PairKey::of(&mine.tree, &other.tree);
                if !state.pairs.contains_key(&key) && state.inflight_pairs.insert(key.clone()) {
                    out.push(OverlapOut::Run(ProbeJob::Pair {
                        key,
                        a: mine.commit.clone(),
                        b: other.commit,
                    }));
                }
            }
            ask_base(&mut state, tab, &mut out);
            out.push(OverlapOut::Updated(tab));
            prune(&mut state);
        }
        OverlapIn::Gone(tab) => {
            if state.agents.remove(&tab).is_some() {
                out.extend(state.agents.keys().map(|t| OverlapOut::Updated(*t)));
                prune(&mut state);
            }
        }
        OverlapIn::Pair(key, probe) => {
            state.inflight_pairs.remove(&key);
            let touched = tabs_with(&state, |snap| snap.tree == key.0 || snap.tree == key.1);
            state.pairs.insert(key, probe);
            out.extend(touched.into_iter().map(OverlapOut::Updated));
        }
        OverlapIn::Base(key, probe) => {
            state.inflight_bases.remove(&key);
            let touched = tabs_with(&state, |snap| snap.tree == key.tree);
            state.bases.insert(key, probe);
            out.extend(touched.into_iter().map(OverlapOut::Updated));
        }
        OverlapIn::Failed(ProbeJob::Pair { key, .. }) => {
            state.inflight_pairs.remove(&key);
        }
        OverlapIn::Failed(ProbeJob::Base { key, .. }) => {
            state.inflight_bases.remove(&key);
        }
    }
    (state, out)
}

fn tabs_with(state: &OverlapState, hit: impl Fn(&Snapshot) -> bool) -> Vec<TabId> {
    state
        .agents
        .iter()
        .filter(|(_, snap)| hit(snap))
        .map(|(tab, _)| *tab)
        .collect()
}

fn ask_base(state: &mut OverlapState, tab: TabId, out: &mut Vec<OverlapOut>) {
    let (Some(base), Some(snap)) = (state.base.clone(), state.agents.get(&tab).cloned()) else {
        return;
    };
    let key = BaseKey {
        tree: snap.tree,
        base: base.clone(),
    };
    if !state.bases.contains_key(&key) && state.inflight_bases.insert(key.clone()) {
        out.push(OverlapOut::Run(ProbeJob::Base {
            key,
            base,
            agent: snap.commit,
        }));
    }
}

/// Drops cached answers no current tree or tip can ask for.
fn prune(state: &mut OverlapState) {
    let trees: BTreeSet<&Oid> = state.agents.values().map(|s| &s.tree).collect();
    state
        .pairs
        .retain(|key, _| trees.contains(&key.0) && trees.contains(&key.1));
    let base = state.base.clone();
    state
        .bases
        .retain(|key, _| trees.contains(&key.tree) && Some(&key.base) == base.as_ref());
}

/// What the tab of `tab` shows now: conflicts (red) first, then files both touch and the base
/// moving (amber), each in file order. Answers still being computed are simply absent.
pub fn overlap_for(state: &OverlapState, tab: TabId) -> AgentOverlap {
    let Some(mine) = state.agents.get(&tab) else {
        return AgentOverlap::default();
    };
    let mut red = Vec::new();
    let mut amber = Vec::new();
    for (other_tab, other) in state.agents.iter().filter(|(t, _)| **t != tab) {
        let Some(probe) = state.pairs.get(&PairKey::of(&mine.tree, &other.tree)) else {
            continue;
        };
        let with = Other::Tab(*other_tab);
        for file in &probe.conflicts {
            red.push(Finding::Conflicts {
                with,
                file: file.clone(),
            });
        }
        for file in probe
            .same_file
            .iter()
            .filter(|f| !probe.conflicts.contains(f))
        {
            amber.push(Finding::BothTouch {
                with,
                file: file.clone(),
            });
        }
    }
    let mut divergence = None;
    let base_probe = state.base.as_ref().and_then(|base| {
        state.bases.get(&BaseKey {
            tree: mine.tree.clone(),
            base: base.clone(),
        })
    });
    if let Some(probe) = base_probe {
        for file in &probe.conflicts {
            red.push(Finding::Conflicts {
                with: Other::Base,
                file: file.clone(),
            });
        }
        if !probe.base_files.is_empty() {
            let overlap = super::probe::intersect(&probe.base_files, &probe.agent_files).len();
            amber.push(Finding::BaseMoved {
                files: probe.base_files.len(),
                overlap,
            });
        }
        divergence = Some(Divergence {
            ahead: probe.ahead,
            behind: probe.behind,
            files_changed: u32::try_from(probe.agent_files.len()).unwrap_or(u32::MAX),
        });
    }
    red.extend(amber);
    AgentOverlap {
        findings: red,
        divergence,
    }
}
