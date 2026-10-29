//! Where a block ran (PLAN P16): the facts about a pane's folder (git branch, uncommitted changes,
//! Python virtualenv, Node version), when to compute them again, and the parsers for what the
//! edge reads. Pure: the edge runs git and reads the files.

mod model;
mod node;
mod parse;
mod python;
mod runtimes;
mod step;
#[cfg(test)]
mod tests;
mod versions;

pub use model::{
    BranchName, DiffStat, EnvName, Facts, MOST_UNTRACKED, NodeVersion, ToolVersion, Untracked,
};
pub use node::{node_from_engines, node_from_nvmrc, node_from_version_file};
pub use parse::{parse_branch, parse_shortstat, parse_untracked, venv_name};
pub use python::{python_from_pyproject, python_from_version_file};
pub use runtimes::{FolderSource, Runtimes, runtimes_in};
pub use step::{FactsIn, FactsOut, FactsState, Flight, facts_step};
pub use versions::{ToolVersions, tool_versions};
