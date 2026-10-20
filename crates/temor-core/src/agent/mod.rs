//! What sits below the agent UI (PLAN §8, P7): the status radar over every pane, D20's approval
//! mode line and D21's context card. All pure; the sources of facts and the model call live at
//! the edges.

pub mod approval;
pub mod context;
pub mod radar;
pub mod rewind;
pub mod seen;
