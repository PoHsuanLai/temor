//! The radar on a virtual clock. No threads, no window.

use temor_core::agent::radar::{Millis, RadarParams};
use temor_core::presence::{AgentState, Seen, WaitKind};
use temor_core::vocab::ids::{ExitCode, PaneId};
use temor_pane::report::{ReportState, ReportWait, StatusLine};
use temor_pane::{CommandLine, PaneEvent, SeenForeground};
use temor_panes::Heard;
use temor_pty::ChildExit;
use temor_vt::{ProgramState, ProgramStatus, ProgramWait};

use super::convert::exit_code;
use super::radar::Radar;

const P1: PaneId = PaneId(1);

pub(super) fn status(state: ProgramState, wait: ProgramWait) -> Heard {
    status_of("claude", state, wait)
}

pub(super) fn status_of(app: &str, state: ProgramState, wait: ProgramWait) -> Heard {
    Heard::Window(PaneEvent::Program(ProgramStatus {
        state,
        wait,
        percent: None,
        record: String::new(),
        app: app.to_owned(),
        title: String::new(),
        message: String::new(),
    }))
}

fn line(seq: u64, state: ReportState, wait: Option<ReportWait>) -> Heard {
    Heard::Window(PaneEvent::Reported(StatusLine {
        seq,
        state,
        wait,
        app: Some("claude".to_owned()),
        note: None,
    }))
}

enum Step {
    Event(Heard),
    Focus(Option<PaneId>),
    Tick,
}

/// Runs `(time in seconds, step)` on pane 1 and returns the state it shows after each step.
fn run(steps: Vec<(u64, Step)>) -> Vec<AgentState> {
    let mut radar = Radar::new(RadarParams::default());
    let mut shown = Vec::new();
    for (secs, step) in steps {
        let now = Millis(secs * 1000);
        match step {
            Step::Event(event) => {
                radar.observe(P1, now, &event);
            }
            Step::Focus(pane) => {
                radar.set_focus(now, pane);
            }
            Step::Tick => {
                radar.tick(now);
            }
        }
        shown.push(radar.state_of(P1));
    }
    shown
}

/// A program in front of the pane, with no command line read.
pub(super) fn front(name: &str) -> Heard {
    Heard::Window(PaneEvent::Foreground(Some(SeenForeground {
        name: name.to_owned(),
        command: None,
    })))
}

/// A wrapper in front of the pane, with the command line it was started with.
pub(super) fn front_running(name: &str, tokens: &[&str]) -> Heard {
    let bytes: Vec<u8> = tokens.iter().flat_map(|t| t.bytes().chain([0])).collect();
    Heard::Window(PaneEvent::Foreground(Some(SeenForeground {
        name: name.to_owned(),
        command: CommandLine::from_cmdline(&bytes),
    })))
}

use AgentState::{Done, Failed, Idle, Waiting, Working};
use Step::{Event, Focus, Tick};

#[test]
fn what_a_pane_says_moves_its_orb_over_time() {
    let spinner = || Heard::Title("\u{280B} fixing tests".to_owned());
    let claude = || front("claude");
    #[rustfmt::skip]
    #[allow(clippy::type_complexity)] // a table of named rows
    let cases: Vec<(&str, Vec<(u64, Step)>, Vec<AgentState>)> = vec![
        ("osc 7501 works, blocks, finishes; the result stays until the pane is focused",
            vec![
                (0, Event(status(ProgramState::Working, ProgramWait::Unspecified))),
                (1, Event(status(ProgramState::Blocked, ProgramWait::Permission))),
                (2, Event(status(ProgramState::Done, ProgramWait::Unspecified))),
                (3, Focus(Some(P1))),
            ],
            vec![Working, Waiting(WaitKind::Permission), Done(Seen::Unseen), Done(Seen::Seen)]),
        ("a bell in a pane that runs no agent says nothing",
            vec![(0, Event(Heard::Bell))], vec![Idle]),
        ("a bell in an agent pane makes it wait until it is focused",
            vec![(0, Event(claude())), (1, Event(Heard::Bell)), (2, Focus(Some(P1)))],
            vec![Idle, Waiting(WaitKind::Question), Idle]),
        ("a notification carries the agent's words and waits the same way",
            vec![
                (0, Event(claude())),
                (1, Event(Heard::Notified { title: "claude".into(), body: "Run tests?".into() })),
            ],
            vec![Idle, Waiting(WaitKind::Question)]),
        ("a spinner title is work for four seconds after it last changed",
            vec![(0, Event(claude())), (1, Event(spinner())), (3, Tick), (6, Tick)],
            vec![Idle, Working, Working, Idle]),
        ("a spinner title means nothing without a known agent",
            vec![(0, Event(spinner()))], vec![Idle]),
        ("a socket report counts, an older one does not",
            vec![
                (0, Event(line(5, ReportState::Waiting, Some(ReportWait::Permission)))),
                (1, Event(line(4, ReportState::Working, None))),
                (2, Event(line(6, ReportState::Done, None))),
            ],
            vec![Waiting(WaitKind::Permission), Waiting(WaitKind::Permission), Done(Seen::Unseen)]),
        ("the program exiting while it works is its result",
            vec![
                (0, Event(status(ProgramState::Working, ProgramWait::Unspecified))),
                (1, Event(Heard::Ended(Some(ChildExit::Exited(0))))),
            ],
            vec![Working, Done(Seen::Unseen)]),
        ("a signal ending it is a failure",
            vec![
                (0, Event(status(ProgramState::Working, ProgramWait::Unspecified))),
                (1, Event(Heard::Ended(Some(ChildExit::Signaled(9))))),
            ],
            vec![Working, Failed(Seen::Unseen)]),
        ("a report in the focused pane is seen as it finishes",
            vec![
                (0, Focus(Some(P1))),
                (1, Event(status(ProgramState::Working, ProgramWait::Unspecified))),
                (2, Event(status(ProgramState::Done, ProgramWait::Unspecified))),
            ],
            vec![Idle, Working, Done(Seen::Seen)]),
    ];
    for (name, steps, want) in cases {
        assert_eq!(run(steps), want, "{name}");
    }
}

#[test]
fn the_waiting_count_and_the_next_waiting_pane_follow_the_radar() {
    let mut radar = Radar::new(RadarParams::default());
    let blocked = status(ProgramState::Blocked, ProgramWait::Question);
    for pane in [PaneId(2), PaneId(5), PaneId(9)] {
        radar.observe(pane, Millis(0), &blocked);
    }
    radar.observe(
        PaneId(9),
        Millis(1),
        &status(ProgramState::Working, ProgramWait::Unspecified),
    );
    assert_eq!(radar.waiting(), [PaneId(2), PaneId(5)]);
    // From pane 2 the next is 5; from 5 it wraps to 2; from a pane that does not wait it is the
    // first after it; with nobody waiting there is none.
    assert_eq!(radar.next_waiting(Some(PaneId(2))), Some(PaneId(5)));
    assert_eq!(radar.next_waiting(Some(PaneId(5))), Some(PaneId(2)));
    assert_eq!(radar.next_waiting(Some(PaneId(3))), Some(PaneId(5)));
    assert_eq!(radar.next_waiting(None), Some(PaneId(2)));
    // A tab shows its most urgent pane.
    let tab = [PaneId(9), PaneId(5), PaneId(7)];
    assert_eq!(radar.most_urgent(tab), Waiting(WaitKind::Question));
    assert_eq!(radar.most_urgent([PaneId(9), PaneId(7)]), Working);
    radar.forget(PaneId(2));
    radar.forget(PaneId(5));
    assert_eq!(radar.next_waiting(None), None);
}

#[test]
fn a_pane_that_needs_you_while_unfocused_asks_for_a_notification_with_its_words() {
    let mut radar = Radar::new(RadarParams::default());
    radar.observe(
        P1,
        Millis(0),
        &Heard::Window(PaneEvent::Reported(StatusLine {
            seq: 1,
            state: ReportState::Waiting,
            wait: Some(ReportWait::Permission),
            app: Some("claude".to_owned()),
            note: Some("Run cargo test?".to_owned()),
        })),
    );
    let alerts = radar.take_alerts();
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].pane, P1);
    assert_eq!(
        alerts[0].note.as_ref().map(|n| n.as_str()),
        Some("Run cargo test?")
    );
    assert!(radar.take_alerts().is_empty(), "taken once");
}

#[test]
fn exits_are_read_as_the_radar_needs() {
    #[rustfmt::skip]
    let exits: &[(Option<ChildExit>, i32)] = &[
        (Some(ChildExit::Exited(0)), 0), (Some(ChildExit::Exited(2)), 2),
        (Some(ChildExit::Signaled(9)), 137), (None, -1),
    ];
    for (exit, want) in exits {
        assert_eq!(exit_code(*exit), ExitCode(*want), "{exit:?}");
    }
}
