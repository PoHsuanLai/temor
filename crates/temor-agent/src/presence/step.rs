//! `presence_step`: one pane's agent state, from reports, focus, prompts and the process.

use super::model::{AgentState, Seen, StatusReport, StatusSource};
use std::collections::BTreeMap;
use temor_core::pane::PaneFocus;
use temor_core::vocab::ids::{ExitCode, Seq};
use temor_core::vocab::text::Note;

/// One pane's agent state, whether it is focused, and the newest report seen from each source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresenceState {
    /// What the agent is doing.
    pub state: AgentState,
    /// Whether the pane has focus.
    pub focus: PaneFocus,
    /// The newest sequence number seen from each source.
    pub newest: BTreeMap<StatusSource, Seq>,
}

impl PresenceState {
    /// A pane with nothing reported yet.
    pub fn new(focus: PaneFocus) -> PresenceState {
        PresenceState {
            state: AgentState::Idle,
            focus,
            newest: BTreeMap::new(),
        }
    }
}

/// What moves a pane's agent state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresenceIn {
    /// A report about this pane.
    Report(StatusReport),
    /// The pane gained or lost focus.
    Focus(PaneFocus),
    /// A new prompt came up (OSC 133 A).
    PromptShown,
    /// The agent's process exited.
    ProcessExited(ExitCode),
}

/// What follows from a change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresenceOut {
    /// The state changed; redraw the orb and the waiting count.
    Changed(AgentState),
    /// The pane needs the person and is not focused: notify with the agent's own words.
    Notify {
        /// Why.
        state: AgentState,
        /// The agent's message.
        note: Option<Note>,
    },
}

/// The pane after `input`. A report older than one already seen from its source is ignored;
/// focus turns Done and Failed into seen; a new prompt clears Working and Waiting, as OSC 7501
/// does.
pub fn presence_step(state: PresenceState, input: PresenceIn) -> (PresenceState, Vec<PresenceOut>) {
    match input {
        PresenceIn::Report(report) => report_step(state, report),
        PresenceIn::Focus(focus) => {
            let next = match (focus, state.state) {
                (PaneFocus::Focused, AgentState::Done(_)) => AgentState::Done(Seen::Seen),
                (PaneFocus::Focused, AgentState::Failed(_)) => AgentState::Failed(Seen::Seen),
                (_, other) => other,
            };
            changed(PresenceState { focus, ..state }, next)
        }
        PresenceIn::PromptShown => {
            let next = match state.state {
                AgentState::Working | AgentState::Waiting(_) => AgentState::Idle,
                other => other,
            };
            changed(state, next)
        }
        PresenceIn::ProcessExited(exit) => {
            let seen = seen_by(state.focus);
            let next = match state.state {
                AgentState::Working | AgentState::Waiting(_) if exit.succeeded() => {
                    AgentState::Done(seen)
                }
                AgentState::Working | AgentState::Waiting(_) => AgentState::Failed(seen),
                other => other,
            };
            let (state, mut out) = changed(state, next);
            if !out.is_empty() {
                out.extend(notify(&state, None));
            }
            (state, out)
        }
    }
}

fn report_step(state: PresenceState, report: StatusReport) -> (PresenceState, Vec<PresenceOut>) {
    let stale = state
        .newest
        .get(&report.source)
        .is_some_and(|seen| !report.seq.is_newer_than(*seen));
    if stale {
        return (state, vec![]);
    }
    let mut newest = state.newest.clone();
    newest.insert(report.source.clone(), report.seq);
    let next = match report.state {
        AgentState::Done(_) => AgentState::Done(seen_by(state.focus)),
        AgentState::Failed(_) => AgentState::Failed(seen_by(state.focus)),
        other => other,
    };
    let (state, mut out) = changed(PresenceState { newest, ..state }, next);
    if !out.is_empty() {
        out.extend(notify(&state, report.note));
    }
    (state, out)
}

/// A finished pane that is focused is seen as it finishes.
fn seen_by(focus: PaneFocus) -> Seen {
    match focus {
        PaneFocus::Focused => Seen::Seen,
        PaneFocus::Unfocused => Seen::Unseen,
    }
}

fn changed(state: PresenceState, next: AgentState) -> (PresenceState, Vec<PresenceOut>) {
    if next == state.state {
        return (state, vec![]);
    }
    (
        PresenceState {
            state: next,
            ..state
        },
        vec![PresenceOut::Changed(next)],
    )
}

/// A notification, only for an unfocused pane that now needs the person.
fn notify(state: &PresenceState, note: Option<Note>) -> Option<PresenceOut> {
    let wants = matches!(
        state.state,
        AgentState::Waiting(_) | AgentState::Done(Seen::Unseen) | AgentState::Failed(Seen::Unseen)
    );
    (wants && state.focus == PaneFocus::Unfocused).then_some(PresenceOut::Notify {
        state: state.state,
        note,
    })
}
