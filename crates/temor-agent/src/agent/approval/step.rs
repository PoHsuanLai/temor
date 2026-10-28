//! `approval_step`: a pane's mode line follows what is read, and a change happens only when the
//! person asks on that pane. A new agent starts from the Space's default, never from the last
//! pane's choice.

use super::model::{ApprovalLine, ApprovalMode, GateFact, LaunchFact, ModeChange, approval_line};

/// One pane's mode line and the change waiting to be confirmed by what is read back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApprovalState {
    /// What the header shows.
    pub line: ApprovalLine,
    /// A change the person asked for that the sources have not shown yet.
    pub requested: Option<ModeChange>,
}

impl ApprovalState {
    /// A pane before anything was read.
    pub const UNREAD: ApprovalState = ApprovalState {
        line: ApprovalLine::NotGated { sandbox: None },
        requested: None,
    };
}

/// What moves the mode line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalIn {
    /// The launch profile and the gate's session were read (at start, on a roster change).
    Read(LaunchFact, GateFact),
    /// The person chose a change on this pane's menu.
    Choose(ModeChange),
}

/// What to carry out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalOut {
    /// Ask the gate to change this pane's session, or restart the agent. Nothing else changes.
    Apply(ModeChange),
}

/// The mode line after `input`. The line changes only on `Read`, never on `Choose`.
pub fn approval_step(state: ApprovalState, input: ApprovalIn) -> (ApprovalState, Vec<ApprovalOut>) {
    match input {
        ApprovalIn::Read(launch, gate) => {
            let line = approval_line(launch, gate);
            let requested = match (state.requested, line) {
                (Some(ModeChange::Asks(want)), ApprovalLine::Gated(m)) if m.asks == want => None,
                (Some(ModeChange::Sandbox(want)), ApprovalLine::Gated(m)) if m.sandbox == want => {
                    None
                }
                (other, _) => other,
            };
            (ApprovalState { line, requested }, vec![])
        }
        ApprovalIn::Choose(change) => {
            let already = matches!(
                (change, state.line),
                (ModeChange::Asks(a), ApprovalLine::Gated(m)) if m.asks == a
            ) || matches!(
                (change, state.line),
                (ModeChange::Sandbox(s), ApprovalLine::Gated(m)) if m.sandbox == s
            );
            match already {
                true => (state, vec![]),
                false => (
                    ApprovalState {
                        requested: Some(change),
                        ..state
                    },
                    vec![ApprovalOut::Apply(change)],
                ),
            }
        }
    }
}

/// The mode a new agent starts with: the Space's default, whatever other panes chose.
pub fn new_agent_mode(space_default: ApprovalMode) -> ApprovalMode {
    space_default
}
