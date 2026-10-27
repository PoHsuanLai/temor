//! Which agent a pane runs and what its command line was, as the radar keeps them for the watch.

use temor_client::model::agent::radar::{Millis, RadarParams};
use temor_client::model::presence::AgentState::Working;
use temor_client::model::vocab::ids::PaneId;
use temor_panes::vt::{ProgramState, ProgramWait};

use super::radar::Radar;
use super::tests::{front, front_running, status, status_of};

#[test]
fn only_named_agents_count_as_agents() {
    let mut radar = Radar::new(RadarParams::default());
    let build = status_of("cargo", ProgramState::Working, ProgramWait::Unspecified);
    radar.observe(PaneId(1), Millis(0), &build);
    radar.observe(
        PaneId(2),
        Millis(0),
        &status(ProgramState::Working, ProgramWait::Unspecified),
    );
    assert_eq!(
        radar.agent_of(PaneId(1)),
        None,
        "a build tool is not an agent"
    );
    assert_eq!(radar.agent_of(PaneId(2)), Some("claude"));
    // Its status still shows on its row.
    assert_eq!(radar.state_of(PaneId(1)), Working);
}

#[test]
fn an_agent_seen_by_its_process_is_gone_when_another_program_is_in_front() {
    let mut radar = Radar::new(RadarParams::default());
    // Seen by what it reported: another program in front does not end it.
    radar.observe(
        PaneId(1),
        Millis(0),
        &status(ProgramState::Idle, ProgramWait::Unspecified),
    );
    radar.observe(PaneId(1), Millis(0), &front("node"));
    assert_eq!(radar.agent_of(PaneId(1)), Some("claude"));
    // Seen in front: the shell coming back ends it.
    radar.observe(PaneId(2), Millis(0), &front("Claude"));
    assert_eq!(radar.agent_of(PaneId(2)), Some("claude"));
    radar.observe(PaneId(2), Millis(1), &front("zsh"));
    assert_eq!(radar.agent_of(PaneId(2)), None);
    // Under a sandbox the name is the wrapper's; the program shows in the command line, and the
    // radar keeps the line for the watch.
    let wrapped = front_running(
        "bwrap",
        &["/usr/bin/bwrap", "--", "claude", "--resume", "x"],
    );
    radar.observe(PaneId(3), Millis(0), &wrapped);
    assert_eq!(radar.agent_of(PaneId(3)), Some("claude"));
    assert_eq!(
        radar.command_of(PaneId(3)).map(|c| c.tokens().len()),
        Some(3)
    );
    radar.observe(PaneId(3), Millis(1), &front("zsh"));
    assert_eq!(radar.agent_of(PaneId(3)), None);
}

#[test]
fn a_status_report_never_takes_away_the_command_line_that_was_seen() {
    let handed_over = front_running(
        "bwrap",
        &[
            "/usr/bin/env",
            "-i",
            "KEY=secret",
            "/usr/bin/bwrap",
            "--",
            "/usr/bin/claude",
            "--resume",
            "x",
        ],
    );
    let report = status(ProgramState::Working, ProgramWait::Unspecified);
    let tokens = |radar: &Radar| {
        radar
            .command_of(PaneId(1))
            .map(|c| c.tokens().to_vec())
            .unwrap_or_default()
    };
    let want = ["/usr/bin/claude", "--resume", "x"];

    // Seen by its process first, then it reports: the line stays, from the program word on.
    let mut radar = Radar::new(RadarParams::default());
    radar.observe(PaneId(1), Millis(0), &handed_over);
    radar.observe(PaneId(1), Millis(1), &report);
    assert_eq!(tokens(&radar), want, "process, then report");

    // It reported first: the process seen afterwards brings the line.
    let mut radar = Radar::new(RadarParams::default());
    radar.observe(PaneId(1), Millis(0), &report);
    assert_eq!(radar.command_of(PaneId(1)), None, "a report has no line");
    radar.observe(PaneId(1), Millis(1), &handed_over);
    assert_eq!(tokens(&radar), want, "report, then process");
}
