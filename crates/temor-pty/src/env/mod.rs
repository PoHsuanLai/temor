//! The child's environment: the variable set, the scrubbed base and temor's contract.

mod contract;
mod terminfo;
#[cfg(test)]
mod tests;

pub use contract::{EnvContract, PaneId, ProgramVersion, base_env, child_env};
pub use terminfo::{SYSTEM_TERMINFO_DIRS, TermName, find_term, terminfo_dirs};
