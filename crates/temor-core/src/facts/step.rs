//! When to compute a pane's facts again. One run is in flight per pane; a trigger that arrives
//! meanwhile is remembered and runs once when the first one is done.

/// Where a pane's facts run stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Flight {
    #[default]
    Idle,
    /// A run is in flight.
    Running,
    /// A run is in flight and something changed since it started: run again after it.
    Stale,
}

/// One pane's state for the rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FactsState {
    pub flight: Flight,
    /// Whether the first prompt has already caused a run.
    pub prompted: bool,
}

/// What happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FactsIn {
    /// The shell reported another folder.
    FolderChanged,
    /// A command finished.
    CommandDone,
    /// A prompt was marked (only the first one counts).
    Prompt,
    /// The run in flight finished.
    RunDone,
}

/// What to do about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FactsOut {
    /// Start a run now.
    Run,
    /// Nothing to start.
    Wait,
}

/// The state after `input`, and whether to start a run.
pub fn facts_step(state: FactsState, input: FactsIn) -> (FactsState, FactsOut) {
    let trigger = |state: FactsState| match state.flight {
        Flight::Idle => (
            FactsState {
                flight: Flight::Running,
                ..state
            },
            FactsOut::Run,
        ),
        Flight::Running | Flight::Stale => (
            FactsState {
                flight: Flight::Stale,
                ..state
            },
            FactsOut::Wait,
        ),
    };
    match input {
        FactsIn::FolderChanged | FactsIn::CommandDone => trigger(state),
        FactsIn::Prompt if !state.prompted => trigger(FactsState {
            prompted: true,
            ..state
        }),
        FactsIn::Prompt => (state, FactsOut::Wait),
        FactsIn::RunDone => match state.flight {
            Flight::Stale => (
                FactsState {
                    flight: Flight::Running,
                    ..state
                },
                FactsOut::Run,
            ),
            Flight::Running | Flight::Idle => (
                FactsState {
                    flight: Flight::Idle,
                    ..state
                },
                FactsOut::Wait,
            ),
        },
    }
}
