//! Radar inputs and outputs: typed facts, no raw sequences.

use crate::presence::{PaneFocus, PresenceOut, StatusReport, WaitKind};
use crate::vocab::ids::ExitCode;
use crate::vocab::text::Note;

/// A time from the caller's clock, in milliseconds. Only differences matter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Millis(pub u64);

impl Millis {
    /// Milliseconds from `earlier` to `self`; zero if the clock stepped back.
    pub fn since(self, earlier: Millis) -> u64 {
        self.0.saturating_sub(earlier.0)
    }
}

/// What a program says it is doing (OSC 7501, already parsed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProgramKind {
    Idle,
    Working,
    Done,
    /// Waiting for the person; `None` when it did not say for what.
    Blocked(Option<WaitKind>),
    Error,
    /// Forget the record (and its children); an empty record forgets all.
    Clear,
}

/// One OSC 7501 report as a fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramFact {
    /// Which record: empty is the program itself, `a/b` is a child of `a`.
    pub record: String,
    pub kind: ProgramKind,
    /// The program's one line for people.
    pub message: Option<Note>,
}

/// Facts about the pane's process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessFact {
    /// A known agent program is running in the pane (from the launch profile or the foreground
    /// process name). Title hints count only while this holds.
    AgentRunning,
    /// The pane's program exited.
    Exited(ExitCode),
}

/// Everything that can move a pane's radar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadarIn {
    /// An OSC 7501 report.
    Program(ProgramFact),
    /// A report from an adapter over the pane socket, or from docket's roster. Reports with the
    /// notification source are the radar's own; callers use `Notified` instead.
    Report(StatusReport),
    /// OSC 9, 99, 777 or a bell, with the text when there was any.
    Notified(Option<Note>),
    /// The window title changed.
    Title(String),
    /// The process changed.
    Process(ProcessFact),
    /// A new prompt came up (OSC 133 A).
    PromptShown,
    /// Focus moved.
    Focus(PaneFocus),
    /// Time passed; call about once a second while any pane is working.
    Tick,
}

/// The timing rules, in one place so they can be tuned without touching the machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadarParams {
    /// A weaker source is ignored while a stronger one reported this recently.
    pub precedence_hold: Millis,
    /// A title that looked busy counts as working for this long after it last changed.
    pub title_hold: Millis,
    /// A working pane whose source has said nothing this long is shown idle again.
    pub stale_working: Millis,
}

impl Default for RadarParams {
    fn default() -> Self {
        RadarParams {
            precedence_hold: Millis(60_000),
            title_hold: Millis(4_000),
            stale_working: Millis(15 * 60_000),
        }
    }
}

/// What follows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadarOut {
    /// What `presence_step` asks for: the state changed, or notify.
    Presence(PresenceOut),
}
