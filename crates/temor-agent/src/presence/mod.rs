//! The agent status radar (§8.1): per-pane agent state, its machine and how it is drawn.

mod display;
mod model;
mod step;
#[cfg(test)]
mod tests;

pub use display::{attention, companion_presence};
pub use model::{AgentState, Seen, StatusReport, StatusSource, WaitKind};
pub use step::{PresenceIn, PresenceOut, PresenceState, presence_step};
pub use temor_core::pane::PaneFocus;
