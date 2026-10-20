//! Watched agents: the turn edges and the rewind rule as tables, and a hub with a fake port that
//! follows an agent from its first sight to its exit.

use temor_core::presence::{AgentState, Seen, WaitKind};
use temor_desktop::{Named, PointId, Rewind, TurnHow, WatchOwner};
use temor_vt::ProgramState;

use super::points::RestoreMenu;
use super::watch::{Edge, edge, rewind_for};
use super::watch_scene::{Scene, in_front, program, sandboxed};
use super::{AgentPick, SheetView};

#[test]
fn a_turn_begins_from_rest_and_ends_into_rest_but_waiting_is_the_middle() {
    use AgentState::{Done, Failed, Idle, Waiting, Working};
    use Seen::Unseen;
    let wait = Waiting(WaitKind::Permission);
    #[rustfmt::skip]
    let cases = [
        ("idle to working", Idle, Working, Some(Edge::Begin)),
        ("a result to working", Done(Unseen), Working, Some(Edge::Begin)),
        ("a failure to working", Failed(Seen::Seen), Working, Some(Edge::Begin)),
        ("waiting to working is the same turn", wait, Working, None),
        ("working to waiting is the same turn", Working, wait, None),
        ("working to idle", Working, Idle, Some(Edge::End(TurnHow::Answered))),
        ("working to done", Working, Done(Unseen), Some(Edge::End(TurnHow::Answered))),
        ("working to failed", Working, Failed(Unseen), Some(Edge::End(TurnHow::Failed))),
        ("waiting to done ends the turn", wait, Done(Unseen), Some(Edge::End(TurnHow::Answered))),
        ("done seen by the person is no edge", Done(Unseen), Done(Seen::Seen), None),
        ("idle stays idle", Idle, Idle, None),
    ];
    for (name, before, after, want) in cases {
        assert_eq!(edge(before, after), want, "{name}");
    }
}

#[test]
fn only_an_agent_that_keeps_its_own_history_is_left_to_it() {
    for (agent, want) in [
        ("claude", Rewind::Agent),
        ("codex", Rewind::Docket),
        ("unknown", Rewind::Docket),
    ] {
        assert_eq!(rewind_for(agent), want, "{agent}");
    }
}

#[test]
fn an_agent_is_watched_from_its_first_sight_to_its_exit() {
    let mut scene = Scene::start();

    // The agent shows up in front of the pane: a watch opens in its folder, and its points are
    // read (two jobs: the open, then the list).
    scene.hear(&in_front("claude"));
    scene.settle(2);
    assert_eq!(scene.fake.writes(), ["watch /work [claude in Shell] Agent"]);

    // It starts working: a point is taken, then the list is read again and says a turn runs.
    scene.hear(&program(ProgramState::Working));
    scene.settle(2);
    assert_eq!(
        scene.fake.writes(),
        ["watch /work [claude in Shell] Agent", "mark w-1"]
    );
    let menu = scene.hub.agent_menu(scene.tab).expect("a menu");
    assert_eq!(menu.restore, RestoreMenu::Waiting);

    // A restore is held back while the turn runs: no plan is asked for.
    scene
        .hub
        .agent_pick(scene.tab, AgentPick::Restore(PointId(1)));
    assert_eq!(scene.hub.notices, ["Wait for the agent to finish"]);
    scene.hub.notices.clear();

    // It finishes: the turn the mark numbered ends, and the list is read again.
    scene.hear(&program(ProgramState::Done));
    scene.settle(2);
    assert_eq!(
        scene.fake.writes(),
        [
            "watch /work [claude in Shell] Agent",
            "mark w-1",
            "ended w-1 7 Answered"
        ]
    );
    let menu = scene.hub.agent_menu(scene.tab).expect("a menu");
    assert!(matches!(menu.restore, RestoreMenu::Lines(_)));

    // The sheet shows the plan only; its Restore… performs with the plan's digest.
    scene
        .hub
        .agent_pick(scene.tab, AgentPick::Restore(PointId(1)));
    scene.settle(1);
    let Some(SheetView::Restore { title, .. }) = scene.hub.agent_sheet() else {
        panic!("a restore sheet");
    };
    assert_eq!(title, "What restoring changes");
    scene.hub.perform_restore();
    scene.settle(2);
    assert!(
        scene
            .fake
            .writes()
            .contains(&"restore w-1 1 digest-1".to_owned())
    );

    // The shell comes back in front: the agent has exited and the watch closes; its restore
    // points stay on offer until the pane ends.
    scene.hear(&in_front("zsh"));
    scene.settle(1);
    assert_eq!(
        scene.fake.writes().last().map(String::as_str),
        Some("close w-1")
    );
    assert!(matches!(
        scene.hub.agent_menu(scene.tab).expect("a menu").restore,
        RestoreMenu::Lines(_)
    ));
}

#[test]
fn a_pane_that_ends_closes_its_watch_and_a_pane_without_a_folder_has_none() {
    let mut scene = Scene::start();
    scene.hear(&in_front("claude"));
    scene.settle(2);
    scene.hub.agent_pane_ended(scene.pane);
    scene.settle(1);
    assert_eq!(
        scene.fake.writes(),
        ["watch /work [claude in Shell] Agent", "close w-1"]
    );

    let mut bare = Scene::start();
    bare.hub.ws.cwds.clear();
    bare.hear(&in_front("codex"));
    bare.hear(&program(ProgramState::Working));
    assert!(bare.fake.calls().is_empty());
}

#[test]
fn a_session_the_assistant_holds_is_marked_but_never_closed() {
    // Each row ends the agent a different way; none of them closes the session.
    for (name, ends) in [("the agent exits", false), ("the pane ends", true)] {
        let mut scene = Scene::start();
        scene.fake.answer_watch(WatchOwner::Companion, Named::App);
        scene.hear(&sandboxed("claude"));
        scene.settle(2);
        scene.hear(&program(ProgramState::Working));
        scene.settle(2);
        match ends {
            false => scene.hear(&in_front("zsh")),
            true => scene.hub.agent_pane_ended(scene.pane),
        }
        assert_eq!(
            scene.fake.writes(),
            ["watch /work [claude in Shell] Agent", "mark w-1"],
            "{name}"
        );
    }
}

#[test]
fn a_recognised_apps_session_outlives_the_agent_until_the_pane_ends_or_another_agent_comes() {
    let mut scene = Scene::start();
    scene.fake.answer_watch(WatchOwner::Terminal, Named::App);
    scene.hear(&sandboxed("claude"));
    scene.settle(2);

    // The agent exits: the session stays open, and the same command picks it up again.
    scene.hear(&in_front("zsh"));
    scene.hear(&sandboxed("claude"));
    assert_eq!(
        scene.fake.writes(),
        ["watch /work [claude in Shell] Agent"],
        "the same agent again"
    );

    // Another agent takes the pane: the old session is closed, a new one opened.
    scene.hear(&in_front("zsh"));
    scene.hear(&sandboxed("codex"));
    scene.settle(1);
    scene.settle(2);
    assert_eq!(
        scene.fake.writes(),
        [
            "watch /work [claude in Shell] Agent",
            "close w-1",
            "watch /work [codex in Shell] Docket"
        ],
        "a different agent"
    );

    // The pane ends: the session is closed.
    scene.hub.agent_pane_ended(scene.pane);
    scene.settle(1);
    assert_eq!(
        scene.fake.writes().last().map(String::as_str),
        Some("close w-1")
    );
}
