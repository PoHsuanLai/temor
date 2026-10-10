//! Context chips (PLAN P16): the facts about a pane's folder (git branch, uncommitted changes,
//! Python virtualenv, Node version), when to compute them again, and the parsers for what the
//! edge reads. Pure: the edge runs git and reads the files.

mod fit;
mod model;
mod node;
mod parse;
mod step;
#[cfg(test)]
mod tests;
mod versions;

pub use fit::{ChipRow, chips_that_fit};
pub use model::{
    BranchName, DiffStat, EnvName, Facts, MOST_UNTRACKED, NodeVersion, ToolVersion, Untracked,
};
pub use node::{node_from_engines, node_from_nvmrc};
pub use parse::{parse_branch, parse_shortstat, parse_untracked, venv_name};
pub use step::{FactsIn, FactsOut, FactsState, Flight, facts_step};
pub use versions::{ToolVersions, node_from_version_file, tool_versions};
