//! A checkpoint port for the hub tests: it answers from fixed data, writes down every call (a
//! watch with the command line it was asked with), and numbers the sessions it opens `w-1`,
//! `w-2`, … so an old session can be told from a new one.

use std::sync::Mutex;

use temor_desktop::{
    CheckpointPort, DesktopError, Named, PlanDigest, PointFault, PointId, PointList, PointPlan,
    RunState, SessionKey, TurnHow, TurnMark, WatchAsk, WatchOwner, Watching,
};

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
    /// How many watches were opened: the next session is `w-` this plus one.
    opened: Mutex<u32>,
}

impl Fake {
    pub(super) fn new(list: PointList, plan: PointPlan) -> Fake {
        Fake {
            list: Mutex::new(list),
            plan,
            calls: Mutex::new(Vec::new()),
            answer: Mutex::new((WatchOwner::Terminal, Named::Unknown)),
            answer_seen: Mutex::new(None),
            opened: Mutex::new(0),
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

    pub(super) fn set_turn(&self, turn: RunState) {
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
        // The command line a watch asked with is written after `|`, as docket gets it.
        let line = seen
            .as_ref()
            .map(|s| format!(" | {}", s.tokens().join(" ")));
        self.log(format!(
            "watch {workspace} [{label}] {rewind:?}{}",
            line.unwrap_or_default()
        ));
        let recognised = *self.answer_seen.lock().unwrap_or_else(|p| p.into_inner());
        let plain = *self.answer.lock().unwrap_or_else(|p| p.into_inner());
        let (owner, named) = recognised.filter(|_| seen.is_some()).unwrap_or(plain);
        let n = {
            let mut opened = self.opened.lock().unwrap_or_else(|p| p.into_inner());
            *opened += 1;
            *opened
        };
        let session = SessionKey::new(&format!("w-{n}"))
            .ok_or(DesktopError::Checkpoint(PointFault::Failed))?;
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
