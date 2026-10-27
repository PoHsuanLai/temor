//! `agent.adapter`: what an adapter reports becomes the pane's agent status, through the same
//! report a program's own status line makes. The host side only: an adapter's words are shown
//! and nothing it says makes temor act (anything that does goes through docket's intents).

use temor_pane::report::{ReportState, ReportWait, StatusLine};
use temor_plugins::message::Phase;
use temor_plugins::tidy::one_line;

/// The longest note kept from an adapter, as for any agent's own words.
const NOTE_CHARS: usize = 200;

/// The status line for `phase`, number `seq` of this run, from the agent called `agent`.
pub fn status_line(seq: u64, phase: Phase, agent: &str, text: &str) -> StatusLine {
    let (state, wait) = match phase {
        Phase::Idle => (ReportState::Idle, None),
        Phase::Working => (ReportState::Working, None),
        Phase::WaitingPermission => (ReportState::Waiting, Some(ReportWait::Permission)),
        Phase::WaitingQuestion => (ReportState::Waiting, Some(ReportWait::Question)),
        Phase::WaitingSignIn => (ReportState::Waiting, Some(ReportWait::SignIn)),
        Phase::Done => (ReportState::Done, None),
        Phase::Failed => (ReportState::Failed, None),
    };
    let note = one_line(text, NOTE_CHARS);
    StatusLine {
        seq,
        state,
        wait,
        app: Some(agent.to_owned()),
        note: (!note.is_empty()).then_some(note),
    }
}

/// What the hub does about an agent that appeared in or left a pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Follow {
    /// The agent did not change.
    Stay,
    /// Stop the pane's adapter, if it has one.
    Stop,
    /// Stop the pane's adapter, if any, and start the one for this agent.
    Restart(String),
}

/// What follows from the pane's agent going from `was` to `is`.
pub fn follow(was: Option<&str>, is: Option<&str>) -> Follow {
    match (was, is) {
        (was, is) if was == is => Follow::Stay,
        (_, Some(agent)) => Follow::Restart(agent.to_owned()),
        (_, None) => Follow::Stop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_phase_is_the_status_a_program_would_report() {
        let table = [
            (Phase::Idle, ReportState::Idle, None),
            (Phase::Working, ReportState::Working, None),
            (
                Phase::WaitingPermission,
                ReportState::Waiting,
                Some(ReportWait::Permission),
            ),
            (
                Phase::WaitingQuestion,
                ReportState::Waiting,
                Some(ReportWait::Question),
            ),
            (
                Phase::WaitingSignIn,
                ReportState::Waiting,
                Some(ReportWait::SignIn),
            ),
            (Phase::Done, ReportState::Done, None),
            (Phase::Failed, ReportState::Failed, None),
        ];
        for (phase, state, wait) in table {
            let line = status_line(7, phase, "claude", "wants to\nrun ls");
            assert_eq!((line.state, line.wait), (state, wait), "{phase:?}");
            assert_eq!(line.seq, 7);
            assert_eq!(line.app.as_deref(), Some("claude"));
            assert_eq!(line.note.as_deref(), Some("wants to run ls"));
        }
        assert_eq!(status_line(1, Phase::Done, "x", " \n").note, None);
    }

    #[test]
    fn an_adapter_follows_the_agent_in_the_pane() {
        let table = [
            ("nothing before or after", None, None, Follow::Stay),
            ("same agent", Some("claude"), Some("claude"), Follow::Stay),
            (
                "an agent appears",
                None,
                Some("claude"),
                Follow::Restart("claude".to_owned()),
            ),
            (
                "another agent",
                Some("claude"),
                Some("codex"),
                Follow::Restart("codex".to_owned()),
            ),
            ("the agent leaves", Some("claude"), None, Follow::Stop),
        ];
        for (why, was, is, want) in table {
            assert_eq!(follow(was, is), want, "{why}");
        }
    }
}
