//! A watch keeps its session through the agent's own status reports: a report says nothing of
//! the command line, so it never closes, replaces or doubles a session the process sighting
//! opened; and a plain session opened before the command line was known is upgraded once.

use temor_desktop::{Named, WatchOwner};
use temor_vt::ProgramState;

use super::watch_scene::{Scene, program, sandboxed};

const WATCH: &str = "watch /work [claude in Shell] Agent";

#[test]
fn a_status_report_after_the_sighting_closes_and_opens_nothing() {
    let mut scene = Scene::start();
    scene.fake.answer_watch(WatchOwner::Terminal, Named::App);
    scene.hear(&sandboxed("claude"));
    scene.quiet();
    // A turn: the report moves the state, the next Begin asks for a watch again.
    for state in [
        ProgramState::Working,
        ProgramState::Done,
        ProgramState::Working,
    ] {
        scene.hear(&program(state));
        scene.quiet();
    }
    assert_eq!(
        scene.fake.writes(),
        [WATCH, "mark w-1", "ended w-1 7 Answered", "mark w-1"]
    );
}

#[test]
fn a_plain_session_opened_before_the_command_line_was_known_is_upgraded_once() {
    let mut scene = Scene::start();
    scene
        .fake
        .answer_watch_seen(WatchOwner::Terminal, Named::App);
    // The agent reports before the foreground poll sees its process: a plain session.
    scene.hear(&program(ProgramState::Working));
    scene.quiet();
    assert_eq!(scene.fake.writes(), [WATCH, "mark w-1"]);

    // The process shows up with its command line: the plain session is closed and a
    // recognised one opened, once.
    scene.hear(&sandboxed("claude"));
    scene.quiet();
    let upgraded = [WATCH, "mark w-1", "close w-1", WATCH];
    assert_eq!(scene.fake.writes(), upgraded, "upgraded");

    // The same line again, and a turn after it, change nothing.
    scene.hear(&sandboxed("claude"));
    scene.hear(&program(ProgramState::Done));
    scene.hear(&program(ProgramState::Working));
    scene.quiet();
    let writes = scene.fake.writes();
    assert_eq!(&writes[..4], upgraded, "no second upgrade");
    assert!(
        writes[4..]
            .iter()
            .all(|call| !call.starts_with("watch ") && !call.starts_with("close ")),
        "{writes:?}"
    );
}

#[test]
fn a_session_the_assistant_holds_is_never_closed_by_a_report_or_a_later_sighting() {
    // Each row opens the session a different way round; the assistant's stays open and single.
    for (name, sighted_first) in [("sighted first", true), ("reported first", false)] {
        let mut scene = Scene::start();
        scene.fake.answer_watch(WatchOwner::Companion, Named::App);
        match sighted_first {
            true => scene.hear(&sandboxed("claude")),
            false => scene.hear(&program(ProgramState::Working)),
        }
        scene.quiet();
        scene.hear(&sandboxed("claude"));
        scene.hear(&program(ProgramState::Done));
        scene.hear(&program(ProgramState::Working));
        scene.quiet();
        let writes = scene.fake.writes();
        let opens = writes.iter().filter(|c| c.starts_with("watch ")).count();
        assert_eq!(opens, 1, "{name}: {writes:?}");
        assert!(
            writes.iter().all(|c| !c.starts_with("close ")),
            "{name}: {writes:?}"
        );
    }
}
