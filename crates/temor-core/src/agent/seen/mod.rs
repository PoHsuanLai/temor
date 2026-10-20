//! Which known agent a foreground process is (PLAN P19): by its process name, or by the program
//! named in its command line when a wrapper such as `bwrap` or `env` runs it.

mod agent;
#[cfg(test)]
mod tests;

pub use agent::{AgentName, agent_of, is_known_agent};
pub use temor_pane::CommandLine;
