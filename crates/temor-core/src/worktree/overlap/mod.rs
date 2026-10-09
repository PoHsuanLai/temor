//! The early overlap check between agent worktrees (D19): deterministic, text only, no AI.

mod model;
mod probe;
mod step;
#[cfg(test)]
mod tests;

pub use model::{AgentOverlap, Divergence, Finding, Other, Severity, TEXT_OVERLAP_ONLY};
pub use probe::{
    BaseProbe, PairProbe, intersect, parse_counts, parse_merge_tree, parse_names, probe_base,
    probe_pair,
};
pub use step::{
    BaseKey, OverlapIn, OverlapOut, OverlapState, PairKey, ProbeJob, overlap_for, overlap_step,
};
