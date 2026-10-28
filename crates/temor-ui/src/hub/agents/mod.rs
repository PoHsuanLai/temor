//! Agents in the window (PLAN §8, P7): the status radar over every pane and the per-pane status
//! sockets. Pure bookkeeping is in `radar`; the rest is the hub's side of it.

mod convert;
mod desk;
mod ending;
mod loops;
mod naming;
#[cfg(test)]
mod naming_tests;
mod observe;
mod radar;
mod state;
#[cfg(test)]
mod tests;

pub use loops::spawn_agent_loops;
pub use observe::WaitingCount;
pub use radar::{Alert, Radar};
pub use state::Agents;
