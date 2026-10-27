//! When a known agent counts as gone from its pane (PLAN §8.1). Its process leaving the front
//! ends an agent seen by its process; an agent seen only by what it reported ends when it clears
//! its status or when a shell comes back to the front. Pure rules; the radar applies them.

/// How a pane's agent was seen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Via {
    /// Its process is in front of the pane.
    Process,
    /// It reported its own status.
    Report,
}

/// What is in front of the pane now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Front {
    /// A shell: whatever ran before has returned to the prompt.
    Shell,
    /// Any other program, or nothing.
    Other,
}

/// Shells, by process name (lowercase).
const SHELLS: &[&str] = &[
    "sh", "bash", "zsh", "fish", "dash", "ksh", "tcsh", "csh", "nu", "nushell", "pwsh", "elvish",
    "xonsh",
];

/// What the foreground process `name` is.
pub(super) fn front_of(name: Option<&str>) -> Front {
    let shell = name.is_some_and(|name| {
        let name = name.trim_start_matches('-').to_ascii_lowercase();
        SHELLS.contains(&name.as_str())
    });
    match shell {
        true => Front::Shell,
        false => Front::Other,
    }
}

impl Via {
    /// Whether an agent seen this way is gone now that `front` is in front. A reporting agent may
    /// run under any program, so only a shell coming back ends it.
    pub(super) fn gone_with(self, front: Front) -> bool {
        matches!(
            (self, front),
            (Via::Process, _) | (Via::Report, Front::Shell)
        )
    }
}

#[cfg(test)]
mod tests {
    use temor_client::model::agent::radar::{Millis, RadarParams};
    use temor_client::model::vocab::ids::PaneId;
    use temor_panes::Heard;
    use temor_panes::pane::PaneEvent;
    use temor_panes::vt::{ProgramState, ProgramStatus, ProgramWait};

    use super::super::radar::Radar;
    use super::*;

    fn report(state: ProgramState) -> Heard {
        Heard::Window(PaneEvent::Program(ProgramStatus {
            state,
            wait: ProgramWait::Unspecified,
            percent: None,
            record: String::new(),
            app: "claude".to_owned(),
            title: String::new(),
            message: String::new(),
        }))
    }

    fn front(name: &str) -> Heard {
        super::super::tests::front(name)
    }

    #[test]
    fn a_name_is_a_shell_or_not() {
        let table = [
            (Some("zsh"), Front::Shell),
            (Some("-bash"), Front::Shell),
            (Some("Fish"), Front::Shell),
            (Some("node"), Front::Other),
            (Some("claude"), Front::Other),
            (Some(""), Front::Other),
            (None, Front::Other),
        ];
        for (name, want) in table {
            assert_eq!(front_of(name), want, "{name:?}");
        }
    }

    #[test]
    fn an_agent_that_only_reported_is_gone_on_clear_or_when_a_shell_returns() {
        // Each row: what happens after a report; whether the agent is still there at the end.
        let rows: Vec<(&str, Vec<Heard>, Option<&str>)> = vec![
            ("nothing", vec![], Some("claude")),
            ("another program", vec![front("node")], Some("claude")),
            ("a shell", vec![front("node"), front("zsh")], None),
            ("cleared", vec![report(ProgramState::Clear)], None),
            (
                "working again after a clear",
                vec![report(ProgramState::Clear), report(ProgramState::Working)],
                Some("claude"),
            ),
        ];
        for (name, events, want) in rows {
            let mut radar = Radar::new(RadarParams::default());
            radar.observe(PaneId(1), Millis(0), &report(ProgramState::Working));
            for (at, event) in events.iter().enumerate() {
                radar.observe(PaneId(1), Millis(1 + at as u64), event);
            }
            assert_eq!(radar.agent_of(PaneId(1)), want, "{name}");
        }
    }

    #[test]
    fn an_agent_seen_by_its_process_outlasts_a_clear() {
        let mut radar = Radar::new(RadarParams::default());
        radar.observe(PaneId(1), Millis(0), &front("claude"));
        radar.observe(PaneId(1), Millis(1), &report(ProgramState::Clear));
        assert_eq!(radar.agent_of(PaneId(1)), Some("claude"));
    }
}
