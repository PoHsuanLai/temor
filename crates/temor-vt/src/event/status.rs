//! OSC 7501 program status reports.

use super::Percent;

/// What a reporting program is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ProgramState {
    Idle,
    Working,
    Done,
    /// Waiting for the person; [`ProgramWait`] says what for.
    Blocked,
    Error,
    /// Forget this record.
    Clear,
}

/// What a blocked program waits for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ProgramWait {
    /// It did not say.
    Unspecified,
    Permission,
    Question,
    Auth,
}

/// One OSC 7501 report, already parsed by libghostty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStatus {
    pub state: ProgramState,
    pub wait: ProgramWait,
    /// Only for `Working` and `Blocked`, when the program sent a value in range.
    pub percent: Option<Percent>,
    /// Which record the report is about; empty for the root. `a/b` is a child of `a`.
    pub record: String,
    /// A stable program name to match on, such as `cargo`.
    pub app: String,
    /// A short label for people.
    pub title: String,
    /// One line for people saying what the record is doing.
    pub message: String,
}
