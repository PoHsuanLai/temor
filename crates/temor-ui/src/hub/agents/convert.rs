//! What the panes say, as the radar's typed facts. Pure: no sequences, no clocks.

use temor_core::agent::radar::{ProgramFact, ProgramKind};
use temor_core::presence::{AgentState, Seen, StatusReport, StatusSource, WaitKind};
use temor_core::vocab::ids::{ExitCode, PaneId, Seq};
use temor_core::vocab::text::{Note, PluginId, display_text};
use temor_pane::report::{ReportState, ReportWait, StatusLine};
use temor_pty::ChildExit;
use temor_vt::{ProgramState, ProgramStatus, ProgramWait};

/// The longest message kept from an agent; it is shown, so it is bounded.
const NOTE_CHARS: usize = 200;

/// Programs temor knows as agents by their process name (lowercase, exact). The weakest evidence:
/// a program's own report (OSC 7501 or the pane socket) counts as well.
const KNOWN_AGENTS: &[&str] = &[
    "claude",
    "codex",
    "gemini",
    "agy",
    "aider",
    "opencode",
    "goose",
    "amp",
    "crush",
    "cursor-agent",
];

/// Whether `name` (a foreground process name) is a known agent program.
pub fn known_agent(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    KNOWN_AGENTS.contains(&name.as_str())
}

fn note(text: &str) -> Option<Note> {
    let text = display_text(text, NOTE_CHARS);
    (!text.is_empty()).then_some(Note(text))
}

/// One OSC 7501 report as the radar's fact.
pub fn program_fact(status: &ProgramStatus) -> ProgramFact {
    let wait = match status.wait {
        ProgramWait::Permission => Some(WaitKind::Permission),
        ProgramWait::Question => Some(WaitKind::Question),
        ProgramWait::Auth => Some(WaitKind::SignIn),
        _ => None,
    };
    let kind = match status.state {
        ProgramState::Working => ProgramKind::Working,
        ProgramState::Done => ProgramKind::Done,
        ProgramState::Blocked => ProgramKind::Blocked(wait),
        ProgramState::Error => ProgramKind::Error,
        ProgramState::Clear => ProgramKind::Clear,
        _ => ProgramKind::Idle,
    };
    ProgramFact {
        record: status.record.clone(),
        kind,
        message: note(&status.message),
    }
}

/// A report from the pane socket as a presence report about `pane`. `Done` and `Failed` carry
/// `Unseen`: `presence_step` sets it from focus.
pub fn status_report(pane: PaneId, line: &StatusLine) -> StatusReport {
    let state = match line.state {
        ReportState::Working => AgentState::Working,
        ReportState::Waiting => AgentState::Waiting(match line.wait {
            Some(ReportWait::Permission) => WaitKind::Permission,
            Some(ReportWait::SignIn) => WaitKind::SignIn,
            _ => WaitKind::Question,
        }),
        ReportState::Done => AgentState::Done(Seen::Unseen),
        ReportState::Failed => AgentState::Failed(Seen::Unseen),
        _ => AgentState::Idle,
    };
    let app = line.app.clone().unwrap_or_else(|| "socket".to_owned());
    StatusReport {
        pane,
        state,
        seq: Seq(line.seq),
        source: StatusSource::Adapter(PluginId(app)),
        note: line.note.as_deref().and_then(note),
    }
}

/// A notification or title text as a note.
pub fn note_of(text: &str) -> Option<Note> {
    note(text)
}

/// How a program ended, as the shell would number it.
pub fn exit_code(exit: Option<ChildExit>) -> ExitCode {
    match exit {
        Some(ChildExit::Exited(code)) => ExitCode(code),
        Some(ChildExit::Signaled(signal)) => ExitCode(128 + signal),
        None => ExitCode(-1),
    }
}
