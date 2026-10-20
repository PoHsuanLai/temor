//! What a turn is, as the watch sees it: the edge between two states of an agent, and who keeps
//! the history of its files.

use temor_core::agent::rewind::{self, rewind_of};
use temor_core::presence::AgentState;
use temor_desktop::{Rewind, TurnHow};

/// What changed about a turn between two states of the same agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Edge {
    /// The agent started a turn.
    Begin,
    /// The agent's turn is over.
    End(TurnHow),
}

/// The turn edge between two states, if there is one. Waiting is the middle of a turn: coming
/// out of it into work is not a new turn.
pub(super) fn edge(before: AgentState, after: AgentState) -> Option<Edge> {
    use AgentState::{Done, Failed, Idle, Waiting, Working};
    match (before, after) {
        (Idle | Done(_) | Failed(_), Working) => Some(Edge::Begin),
        (Working | Waiting(_), Idle | Done(_)) => Some(Edge::End(TurnHow::Answered)),
        (Working | Waiting(_), Failed(_)) => Some(Edge::End(TurnHow::Failed)),
        _ => None,
    }
}

/// What docket is told about who keeps the history of the agent program `agent`.
pub(super) fn rewind_for(agent: &str) -> Rewind {
    match rewind_of(agent) {
        rewind::Rewind::Own => Rewind::Agent,
        rewind::Rewind::Docket => Rewind::Docket,
    }
}
