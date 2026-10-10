//! Context chips (PLAN P16): the facts about a pane's folder (git branch, uncommitted changes,
//! Python virtualenv, Node version), when to compute them again, and the parsers for what the
//! edge reads. Pure: the edge runs git and reads the files.

mod model;
mod node;
mod parse;
mod step;
#[cfg(test)]
mod tests;

pub use model::{BranchName, DiffStat, EnvName, Facts, NodeVersion};
pub use node::{node_from_engines, node_from_nvmrc};
pub use parse::{parse_branch, parse_shortstat, venv_name};
pub use step::{FactsIn, FactsOut, FactsState, Flight, facts_step};
