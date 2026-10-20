//! A watch keeps its session through the agent's own status reports: a report says nothing of
//! the command line, so it never closes, replaces or doubles a session the process sighting
//! opened; and a plain session opened before the command line was known is upgraded once, when
//! no turn runs.

use temor_desktop::{Named, WatchOwner};
use temor_vt::ProgramState;

use super::watch_scene::{CLAUDE_SEEN, Scene, in_front, program, sandboxed};

/// The plain watch a status report asks for: no command line.
const PLAIN: &str = "watch /work [claude in Shell] Agent";

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
        [CLAUDE_SEEN, "mark w-1", "ended w-1 7 Answered", "mark w-1"]
    );
}

#[test]
fn a_plain_session_is_upgraded_once_after_the_turn_it_was_marked_in() {
    let mut scene = Scene::start();
    scene
        .fake
        .answer_watch_seen(WatchOwner::Terminal, Named::App);
    // The agent reports before the foreground poll sees its process: a plain session.
    scene.hear(&program(ProgramState::Working));
    scene.quiet();
    assert_eq!(scene.fake.writes(), [PLAIN, "mark w-1"]);

    // The process shows up with its command line mid-turn: the turn ends on the session it was
    // marked on, and only the next turn opens the recognised session.
    scene.hear(&sandboxed("claude"));
    scene.quiet();
    assert_eq!(scene.fake.writes(), [PLAIN, "mark w-1"], "mid-turn");
    scene.hear(&program(ProgramState::Done));
    scene.hear(&program(ProgramState::Working));
    scene.quiet();
    let upgraded = [
        PLAIN,
        "mark w-1",
        "ended w-1 7 Answered",
        "close w-1",
        CLAUDE_SEEN,
        "mark w-2",
    ];
    assert_eq!(scene.fake.writes(), upgraded, "upgraded");

    // The same line again, and a turn after it, change nothing.
    scene.hear(&sandboxed("claude"));
    scene.hear(&program(ProgramState::Done));
    scene.hear(&program(ProgramState::Working));
    scene.quiet();
    let writes = scene.fake.writes();
    assert_eq!(&writes[..6], upgraded, "no second upgrade");
    assert!(
        writes[6..]
            .iter()
            .all(|call| !call.starts_with("watch ") && !call.starts_with("close ")),
        "{writes:?}"
    );
}

#[test]
fn a_turn_whose_mark_has_not_come_back_is_still_mid_turn() {
    // The command line shows up while the turn's mark is still queued behind the list read, or
    // still on its way: the plain session is not closed before the turn ends on it.
    for (name, settled) in [("mark queued", 1), ("mark in flight", 2)] {
        let mut scene = Scene::start();
        scene
            .fake
            .answer_watch_seen(WatchOwner::Terminal, Named::App);
        scene.hear(&in_front("claude"));
        scene.settle(settled);
        scene.hear(&program(ProgramState::Working));
        scene.hear(&sandboxed("claude"));
        scene.quiet();
        assert_eq!(scene.fake.writes(), [PLAIN, "mark w-1"], "{name}: mid-turn");
        scene.hear(&program(ProgramState::Done));
        scene.hear(&program(ProgramState::Working));
        scene.quiet();
        assert_eq!(
            scene.fake.writes(),
            [
                PLAIN,
                "mark w-1",
                "ended w-1 7 Answered",
                "close w-1",
                CLAUDE_SEEN,
                "mark w-2",
            ],
            "{name}: upgraded after the turn"
        );
    }
}

#[test]
fn a_session_the_assistant_holds_is_never_closed_by_a_report_or_a_later_sighting() {
    // docket answers the assistant's session only to a watch that names the app (a plain watch
    // is the window's own). Each row opens the watch a different way round; the assistant's
    // session is never closed, even when the pane ends.
    #[rustfmt::skip]
    let rows: [(&str, bool, &[&str]); 2] = [
        ("sighted first", true, &[CLAUDE_SEEN, "mark w-1"]),
        ("reported first", false, &[
            PLAIN, "mark w-1", "ended w-1 7 Answered", "close w-1", CLAUDE_SEEN, "mark w-2",
        ]),
    ];
    for (name, sighted_first, want) in rows {
        let mut scene = Scene::start();
        scene
            .fake
            .answer_watch_seen(WatchOwner::Companion, Named::App);
        match sighted_first {
            true => scene.hear(&sandboxed("claude")),
            false => scene.hear(&program(ProgramState::Working)),
        }
        scene.quiet();
        scene.hear(&sandboxed("claude"));
        scene.hear(&program(ProgramState::Done));
        scene.hear(&program(ProgramState::Working));
        scene.quiet();
        scene.hub.agent_pane_ended(scene.pane);
        scene.quiet();
        assert_eq!(scene.fake.writes(), want, "{name}");
    }
}
