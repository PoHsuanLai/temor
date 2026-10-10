//! Watched agents: the turn edges and the rewind rule as tables, and a hub with a fake port that
//! follows an agent from its first sight to its exit.

use std::sync::Arc;
use std::time::{Duration, Instant};

use temor_core::presence::{AgentState, Seen, WaitKind};
use temor_core::vocab::ids::{PaneId, TabId};
use temor_core::vocab::text::Cwd;
use temor_core::workspace::WorkspaceIn;
use temor_desktop::{PointId, Rewind, TurnHow};
use temor_pane::PaneEvent;
use temor_vt::{ProgramState, ProgramStatus, ProgramWait};
use tokio::sync::mpsc::UnboundedReceiver;

use super::jobs::Done;
use super::points::RestoreMenu;
use super::points_tests::{Fake, first_list, first_plan};
use super::watch::{Edge, edge, rewind_for};
use super::{AgentPick, SheetView};
use crate::hub::model::Hub;
use crate::hub::tests::hub_after;

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

struct Scene {
    hub: Hub,
    inbox: UnboundedReceiver<Done>,
    fake: Arc<Fake>,
    tab: TabId,
    pane: PaneId,
}

fn program(state: ProgramState) -> PaneEvent {
    PaneEvent::Program(ProgramStatus {
        state,
        wait: ProgramWait::Unspecified,
        percent: None,
        record: String::new(),
        app: "claude".to_owned(),
        title: String::new(),
        message: String::new(),
    })
}

fn in_front(name: &str) -> PaneEvent {
    PaneEvent::Foreground(Some(name.to_owned()))
}

impl Scene {
    /// A hub with one tab in `/work`, over a fake port.
    fn start() -> Scene {
        let mut hub = hub_after(vec![WorkspaceIn::NewTab { cwd: None }]);
        let tab = hub.ws.front().expect("a tab");
        let pane = hub.ws.focus_of(tab).expect("a pane");
        hub.ws.cwds.insert(pane, Cwd("/work".to_owned()));
        let inbox = hub.agents.jobs.take_inbox().expect("the inbox");
        let fake = Arc::new(Fake::new(first_list(), first_plan("digest-1")));
        hub.set_checkpoints(fake.clone());
        Scene {
            hub,
            inbox,
            fake,
            tab,
            pane,
        }
    }

    fn hear(&mut self, event: &PaneEvent) {
        self.hub.agents_observe(self.pane, event);
    }

    /// Carries on from the next `count` finished jobs, waiting up to ten seconds for each.
    fn settle(&mut self, count: usize) {
        for _ in 0..count {
            let until = Instant::now() + Duration::from_secs(10);
            let done = loop {
                match self.inbox.try_recv() {
                    Ok(done) => break done,
                    Err(_) if Instant::now() < until => {
                        std::thread::sleep(Duration::from_millis(2))
                    }
                    Err(_) => panic!("a job did not report"),
                }
            };
            self.hub.agent_done(done);
        }
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

    // The shell comes back in front: the agent has exited and the watch closes.
    scene.hear(&in_front("zsh"));
    scene.settle(1);
    assert_eq!(
        scene.fake.writes().last().map(String::as_str),
        Some("close w-1")
    );
    assert_eq!(
        scene.hub.agent_menu(scene.tab).expect("a menu").restore,
        RestoreMenu::Absent
    );
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
