//! Restore points in the hub with a fake port: the list shown in the menu, the plan shown in the
//! sheet, the restore performed with the plan's digest, and Restore held back while docket's list
//! says a turn is running. Each test waits for the job it started.

use std::sync::{Arc, Mutex};

use temor_core::vocab::ids::{PaneId, TabId};
use temor_core::workspace::WorkspaceIn;
use temor_desktop::{
    CheckpointPort, DesktopError, Named, NotSavedWhy, PlanDigest, PointFault, PointId, PointList,
    PointPlan, PointRow, PointState, RunState, SessionKey, TurnHow, TurnMark, TurnNo, WatchAsk,
    WatchOwner, Watching,
};
use temor_ssh::UnixSeconds;
use tokio::sync::mpsc::UnboundedReceiver;

use super::jobs::Done;
use super::points::{PointLine, RestoreMenu, WAIT_FOR_AGENT, restore_menu};
use super::{AgentPick, SheetView};
use crate::hub::model::Hub;
use crate::hub::tests::hub_after;

const AT: UnixSeconds = UnixSeconds(1_790_000_000);

fn saved(id: u32, turn: u64, state: PointState) -> PointRow {
    PointRow::Saved {
        id: PointId(id),
        turn: TurnNo(turn),
        at: AT,
        state,
    }
}

fn not_saved(turn: u64, why: NotSavedWhy) -> PointRow {
    PointRow::NotSaved {
        turn: TurnNo(turn),
        at: AT,
        why,
    }
}

fn choice(id: u32, label: &str) -> PointLine {
    PointLine::Choice {
        id: PointId(id),
        label: label.to_owned(),
    }
}

fn note(text: &str) -> PointLine {
    PointLine::Note(text.to_owned())
}

#[test]
fn the_menu_lists_the_points_and_says_why_a_turn_has_none() {
    use PointState::{Available, Gone};
    let list = |rows: Vec<PointRow>| {
        Some(PointList {
            rows,
            turn: RunState::Idle,
        })
    };
    let running = Some(PointList {
        rows: vec![saved(1, 1, Available)],
        turn: RunState::Running,
    });
    let many: Vec<PointRow> = (1..=15)
        .map(|n| saved(n, u64::from(n), Available))
        .collect();
    #[rustfmt::skip]
    let cases = vec![
        ("no list yet", None, RestoreMenu::Absent),
        ("an empty list", list(vec![]), RestoreMenu::Absent),
        ("a turn is running", running, RestoreMenu::Waiting),
        ("a point", list(vec![saved(1, 1, Available)]),
            RestoreMenu::Lines(vec![choice(1, "Before turn 1")])),
        ("a cleared point is a note", list(vec![saved(1, 1, Gone)]),
            RestoreMenu::Lines(vec![note("Before turn 1: cleared")])),
        ("a folder without history says so",
            list(vec![not_saved(2, NotSavedWhy::NoHistory)]),
            RestoreMenu::Lines(vec![note("Turn 2 not saved: this folder is not in a git repository")])),
        ("an agent with its own history",
            list(vec![not_saved(3, NotSavedWhy::AgentKeepsOwn)]),
            RestoreMenu::Lines(vec![note("Turn 3 not saved: this agent keeps its own history")])),
        ("a restore names the turn it went back to",
            list(vec![saved(1, 4, Available), PointRow::RestoredTo { id: PointId(1), at: AT }]),
            RestoreMenu::Lines(vec![choice(1, "Before turn 4"), note("Put back to before turn 4")])),
    ];
    for (name, list, want) in cases {
        assert_eq!(restore_menu(list.as_ref()), want, "{name}");
    }
    // The newest twelve are shown.
    let all = PointList {
        rows: many,
        turn: RunState::Idle,
    };
    let RestoreMenu::Lines(lines) = restore_menu(Some(&all)) else {
        panic!("lines");
    };
    assert_eq!(lines.len(), 12);
    assert_eq!(lines[0], choice(4, "Before turn 4"));
}

/// A port that answers from fixed data and writes down every call.
#[derive(Debug)]
pub(super) struct Fake {
    list: Mutex<PointList>,
    plan: PointPlan,
    calls: Mutex<Vec<String>>,
    /// What a watch answers: who holds its session, and whether the app was recognised.
    answer: Mutex<(WatchOwner, Named)>,
    /// What a watch asked with a command line answers instead, when set (a recognised app).
    answer_seen: Mutex<Option<(WatchOwner, Named)>>,
}

impl Fake {
    pub(super) fn new(list: PointList, plan: PointPlan) -> Fake {
        Fake {
            list: Mutex::new(list),
            plan,
            calls: Mutex::new(Vec::new()),
            answer: Mutex::new((WatchOwner::Terminal, Named::Unknown)),
            answer_seen: Mutex::new(None),
        }
    }

    /// From now on a watch answers with a session `owner` closes, for an app that is `named`.
    pub(super) fn answer_watch(&self, owner: WatchOwner, named: Named) {
        *self.answer.lock().unwrap_or_else(|p| p.into_inner()) = (owner, named);
    }

    /// From now on a watch asked with a command line answers with a session `owner` closes, for
    /// an app that is `named`; one asked without keeps the plain answer.
    pub(super) fn answer_watch_seen(&self, owner: WatchOwner, named: Named) {
        *self.answer_seen.lock().unwrap_or_else(|p| p.into_inner()) = Some((owner, named));
    }

    fn log(&self, call: String) {
        self.calls
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(call);
    }

    pub(super) fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// The calls other than reading the list: those come in an order the threads decide.
    pub(super) fn writes(&self) -> Vec<String> {
        let mut calls = self.calls();
        calls.retain(|call| !call.starts_with("list "));
        calls
    }

    fn set_turn(&self, turn: RunState) {
        self.list.lock().unwrap_or_else(|p| p.into_inner()).turn = turn;
    }
}

impl CheckpointPort for Fake {
    fn list(&self, session: &SessionKey) -> Result<PointList, DesktopError> {
        self.log(format!("list {}", session.as_str()));
        Ok(self.list.lock().unwrap_or_else(|p| p.into_inner()).clone())
    }

    fn plan(&self, session: &SessionKey, id: PointId) -> Result<PointPlan, DesktopError> {
        self.log(format!("plan {} {}", session.as_str(), id.0));
        Ok(self.plan.clone())
    }

    fn restore(
        &self,
        session: &SessionKey,
        id: PointId,
        digest: &PlanDigest,
    ) -> Result<(), DesktopError> {
        self.log(format!(
            "restore {} {} {}",
            session.as_str(),
            id.0,
            digest.0
        ));
        match digest.0.as_str() {
            "stale" => Err(DesktopError::Checkpoint(PointFault::PlanStale)),
            _ => Ok(()),
        }
    }

    fn watch(&self, ask: &WatchAsk) -> Result<Watching, DesktopError> {
        let WatchAsk {
            workspace,
            label,
            rewind,
            seen,
        } = ask;
        self.log(format!("watch {workspace} [{label}] {rewind:?}"));
        let recognised = *self.answer_seen.lock().unwrap_or_else(|p| p.into_inner());
        let plain = *self.answer.lock().unwrap_or_else(|p| p.into_inner());
        let (owner, named) = recognised.filter(|_| seen.is_some()).unwrap_or(plain);
        let session = SessionKey::new("w-1").ok_or(DesktopError::Checkpoint(PointFault::Failed))?;
        Ok(Watching {
            session,
            owner,
            named,
        })
    }

    fn mark(&self, session: &SessionKey) -> Result<TurnMark, DesktopError> {
        self.log(format!("mark {}", session.as_str()));
        self.set_turn(RunState::Running);
        Ok(TurnMark(7))
    }

    fn turn_ended(
        &self,
        session: &SessionKey,
        turn: TurnMark,
        how: TurnHow,
    ) -> Result<(), DesktopError> {
        self.log(format!("ended {} {} {how:?}", session.as_str(), turn.0));
        self.set_turn(RunState::Idle);
        Ok(())
    }

    fn close(&self, session: &SessionKey) -> Result<(), DesktopError> {
        self.log(format!("close {}", session.as_str()));
        Ok(())
    }
}

/// The list a fake starts with: a point, and a turn that was not saved.
pub(super) fn first_list() -> PointList {
    PointList {
        rows: vec![
            saved(1, 1, PointState::Available),
            not_saved(2, NotSavedWhy::NoHistory),
        ],
        turn: RunState::Idle,
    }
}

pub(super) fn first_plan(digest: &str) -> PointPlan {
    PointPlan {
        changed: vec!["a.txt".to_owned()],
        added: vec!["b.txt".to_owned()],
        removed: vec!["c.txt".to_owned()],
        digest: PlanDigest(digest.to_owned()),
    }
}

struct Scene {
    hub: Hub,
    inbox: UnboundedReceiver<Done>,
    fake: Arc<Fake>,
    tab: TabId,
    pane: PaneId,
}

impl Scene {
    /// A hub with one tab whose pane runs session `s-7`, its list already read.
    fn start(digest: &str) -> Scene {
        let mut hub = hub_after(vec![WorkspaceIn::NewTab { cwd: None }]);
        let tab = hub.ws.front().expect("a tab");
        let pane = hub.ws.focus_of(tab).expect("a pane");
        let inbox = hub.agents.jobs.take_inbox().expect("the inbox");
        let fake = Arc::new(Fake::new(first_list(), first_plan(digest)));
        hub.set_checkpoints(fake.clone());
        let mut scene = Scene {
            hub,
            inbox,
            fake,
            tab,
            pane,
        };
        scene.hub.set_agent_session(pane, SessionKey::new("s-7"));
        scene.settle();
        scene
    }

    /// Waits for the job in flight and carries on from it.
    fn settle(&mut self) {
        let done = self.inbox.blocking_recv().expect("a job reports");
        self.hub.agent_done(done);
    }
}

#[test]
fn a_restore_shows_the_list_then_the_plan_then_performs_with_the_plans_digest() {
    let mut scene = Scene::start("digest-1");
    let menu = scene.hub.agent_menu(scene.tab).expect("a menu");
    assert_eq!(
        menu.restore,
        RestoreMenu::Lines(vec![
            choice(1, "Before turn 1"),
            note("Turn 2 not saved: this folder is not in a git repository"),
        ])
    );

    scene
        .hub
        .agent_pick(scene.tab, AgentPick::Restore(PointId(1)));
    scene.settle();
    assert_eq!(
        scene.hub.agent_sheet(),
        Some(SheetView::Restore {
            title: "What restoring changes".to_owned(),
            put_back: vec!["a.txt".to_owned()],
            bring_back: vec!["b.txt".to_owned()],
            remove: vec!["c.txt".to_owned()],
            note: "Files git ignores are not touched",
        })
    );

    scene.hub.perform_restore();
    scene.settle();
    // The restore is done; the list is read again to show it.
    scene.settle();
    assert_eq!(
        scene.fake.calls(),
        [
            "list s-7",
            "plan s-7 1",
            "restore s-7 1 digest-1",
            "list s-7"
        ]
    );
    assert_eq!(scene.hub.agent_sheet(), None);
    assert_eq!(
        scene.hub.notices,
        ["Files are back as they were before turn 1"]
    );
}

#[test]
fn a_stale_plan_is_refused_in_the_persons_words() {
    let mut scene = Scene::start("stale");
    scene
        .hub
        .agent_pick(scene.tab, AgentPick::Restore(PointId(1)));
    scene.settle();
    scene.hub.perform_restore();
    scene.settle();
    assert_eq!(
        scene.hub.notices,
        [format!(
            "The files could not be put back: {}",
            PointFault::PlanStale.words()
        )]
    );
}

#[test]
fn restore_is_held_back_while_the_list_says_a_turn_is_running() {
    let mut scene = Scene::start("digest-1");
    scene
        .hub
        .agent_pick(scene.tab, AgentPick::Restore(PointId(1)));
    scene.settle();
    assert!(scene.hub.agent_sheet().is_some());

    scene.fake.set_turn(RunState::Running);
    scene.hub.refresh_points(scene.pane);
    scene.settle();
    // The menu shows Restore as waiting, and a pick asks nothing of docket.
    let menu = scene.hub.agent_menu(scene.tab).expect("a menu");
    assert_eq!(menu.restore, RestoreMenu::Waiting);
    scene
        .hub
        .agent_pick(scene.tab, AgentPick::Restore(PointId(1)));
    // A sheet that was open before the turn began is not performed either.
    scene.hub.perform_restore();
    assert_eq!(scene.hub.notices, [WAIT_FOR_AGENT, WAIT_FOR_AGENT]);
    assert_eq!(scene.fake.calls(), ["list s-7", "plan s-7 1", "list s-7"]);
}
