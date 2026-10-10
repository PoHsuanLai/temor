//! The approval mode line (D20): two facts on every agent pane, read from the launch profile and
//! the gate, never from what the agent says.

mod model;
mod step;
#[cfg(test)]
mod tests;

pub use model::{
    ApprovalLine, ApprovalMode, AsksLevel, GateFact, LaunchFact, ModeChange, SandboxMode,
    approval_line, tooltip,
};
pub use step::{ApprovalIn, ApprovalOut, ApprovalState, approval_step, new_agent_mode};
