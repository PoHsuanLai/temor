//! The radar on a virtual clock, the worktree look timing and the
//! path rule for files put back. No threads, no git, no window.

use std::path::Path;

use temor_core::agent::radar::{Millis, RadarParams};
use temor_core::presence::{AgentState, Seen, WaitKind};
use temor_core::vocab::ids::{ExitCode, PaneId, TabId};
use temor_core::worktree::{AgentWorktree, Oid, WorktreeName};
use temor_pane::report::{ReportState, ReportWait, StatusLine};
use temor_pane::{CommandLine, PaneEvent, SeenForeground};
use temor_pty::ChildExit;
use temor_vt::{Notification, ProgramState, ProgramStatus, ProgramWait};

use super::convert::exit_code;
use super::radar::Radar;
use super::trees::Trees;
use super::work::safe_join;

const P1: PaneId = PaneId(1);

fn status(state: ProgramState, wait: ProgramWait) -> PaneEvent {
    status_of("claude", state, wait)
}

fn status_of(app: &str, state: ProgramState, wait: ProgramWait) -> PaneEvent {
    PaneEvent::Program(ProgramStatus {
        state,
        wait,
        percent: None,
        record: String::new(),
        app: app.to_owned(),
        title: String::new(),
        message: String::new(),
    })
}

fn line(seq: u64, state: ReportState, wait: Option<ReportWait>) -> PaneEvent {
    PaneEvent::Reported(StatusLine {
        seq,
        state,
        wait,
        app: Some("claude".to_owned()),
        note: None,
    })
}

enum Step {
    Event(PaneEvent),
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
pub(super) fn front(name: &str) -> PaneEvent {
    PaneEvent::Foreground(Some(SeenForeground {
        name: name.to_owned(),
        command: None,
    }))
}

/// A wrapper in front of the pane, with the command line it was started with.
pub(super) fn front_running(name: &str, tokens: &[&str]) -> PaneEvent {
    let bytes: Vec<u8> = tokens.iter().flat_map(|t| t.bytes().chain([0])).collect();
    PaneEvent::Foreground(Some(SeenForeground {
        name: name.to_owned(),
        command: CommandLine::from_cmdline(&bytes),
    }))
}

use AgentState::{Done, Failed, Idle, Waiting, Working};
use Step::{Event, Focus, Tick};

#[test]
fn what_a_pane_says_moves_its_orb_over_time() {
    let spinner = || PaneEvent::Title("\u{280B} fixing tests".to_owned());
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
            vec![(0, Event(PaneEvent::Bell))], vec![Idle]),
        ("a bell in an agent pane makes it wait until it is focused",
            vec![(0, Event(claude())), (1, Event(PaneEvent::Bell)), (2, Focus(Some(P1)))],
            vec![Idle, Waiting(WaitKind::Question), Idle]),
        ("a notification carries the agent's words and waits the same way",
            vec![
                (0, Event(claude())),
                (1, Event(PaneEvent::Notification(Notification { title: "claude".into(), body: "Run tests?".into() }))),
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
                (1, Event(PaneEvent::Exited(Some(ChildExit::Exited(0))))),
            ],
            vec![Working, Done(Seen::Unseen)]),
        ("a signal ending it is a failure",
            vec![
                (0, Event(status(ProgramState::Working, ProgramWait::Unspecified))),
                (1, Event(PaneEvent::Exited(Some(ChildExit::Signaled(9))))),
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
        &PaneEvent::Reported(StatusLine {
            seq: 1,
            state: ReportState::Waiting,
            wait: Some(ReportWait::Permission),
            app: Some("claude".to_owned()),
            note: Some("Run cargo test?".to_owned()),
        }),
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
        Some(5)
    );
    radar.observe(PaneId(3), Millis(1), &front("zsh"));
    assert_eq!(radar.agent_of(PaneId(3)), None);
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

fn worktree(path: &str) -> AgentWorktree {
    AgentWorktree {
        tab: TabId(0),
        name: WorktreeName::from_task("fix login").expect("name"),
        repo: "/r".to_owned(),
        path: path.to_owned(),
        base_ref: "main".to_owned(),
        base: Oid::parse(&format!("{:040x}", 1)).expect("oid"),
    }
}

#[test]
fn a_worktree_is_looked_at_once_after_its_commands_settle() {
    let mut trees = Trees::default();
    trees.adopt(TabId(4), worktree("/d/w"), Millis(10_000));
    // The first look is due at once.
    assert_eq!(trees.due(Millis(10_000)).len(), 1);
    assert!(trees.due(Millis(11_000)).is_empty(), "one look at a time");
    let tip = Oid::parse(&format!("{:040x}", 2)).expect("tip");
    let snap = temor_core::worktree::Snapshot {
        commit: tip.clone(),
        tree: tip.clone(),
    };
    trees.snapshotted(TabId(4), tip, snap);
    // A burst of commands counts once, and only after two quiet seconds.
    trees.command_done(TabId(4), Millis(20_000));
    trees.command_done(TabId(4), Millis(20_500));
    assert!(trees.due(Millis(21_000)).is_empty());
    assert_eq!(trees.due(Millis(22_000)).len(), 1);
    // A failed look is asked for again after the next command, not at once.
    trees.snapshot_failed(TabId(4));
    assert!(trees.due(Millis(40_000)).is_empty());
    trees.command_done(TabId(4), Millis(40_000));
    assert_eq!(trees.due(Millis(42_000)).len(), 1);
    trees.gone(TabId(4));
    assert!(trees.by_tab.is_empty());
}

#[test]
fn files_are_put_back_only_inside_the_repository() {
    let root = Path::new("/r");
    #[rustfmt::skip]
    let rows: &[(&str, Option<&str>)] = &[
        ("a.txt", Some("/r/a.txt")),
        ("src/deep/b.rs", Some("/r/src/deep/b.rs")),
        ("../escape", None),
        ("src/../../escape", None),
        ("/etc/passwd", None),
        ("./a", None),
        ("", None),
    ];
    for (rel, want) in rows {
        assert_eq!(
            safe_join(root, rel).as_deref(),
            want.map(Path::new),
            "{rel:?}"
        );
    }
}
