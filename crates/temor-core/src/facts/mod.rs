//! The facts about a pane's folder (PLAN P16): the Python virtualenv and the runtime versions a
//! project asks for, when to compute them again, and the rules for what the edge reads. Pure: the
//! edge reads the files. temor reads no git: what a repository says is a plugin's to tell.

mod model;
mod node;
mod parse;
mod python;
mod runtimes;
mod step;
#[cfg(test)]
mod tests;
mod versions;

pub use model::{EnvName, Facts, NodeVersion, ToolVersion};
pub use node::{node_from_engines, node_from_nvmrc, node_from_version_file};
pub use parse::venv_name;
pub use python::{python_from_pyproject, python_from_version_file};
pub use runtimes::{FolderSource, Runtimes, runtimes_in};
pub use step::{FactsIn, FactsOut, FactsState, Flight, facts_step};
pub use versions::{ToolVersions, tool_versions};
