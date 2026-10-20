//! docket's restore points over the session bus: the list and the plan are `Checkpoint.List` and
//! `Checkpoint.Plan`; the restore is the destructive action `checkpoints.restore` of
//! `org.quire.Checkpoints`, performed with the digest of the plan the person saw. docket draws
//! the confirmation where its policy says (the action asks every time).

use super::bus::connect;
use crate::checkpoints::{
    CheckpointPort, Named, NotSavedWhy, PlanDigest, PointFault, PointId, PointList, PointPlan,
    PointRow, PointState, Rewind, RunState, SessionKey, TurnHow, TurnMark, TurnNo, WatchAsk,
    WatchOwner, Watching,
};
use crate::{DesktopEnv, DesktopError};
use docket_client::{ClientError, DbusTransport, Intents};
use docket_core::{
    ActionRef, Args, CallRefusal, CallRequest, CheckpointFault, CheckpointId, CheckpointRow,
    Origin, ParamName, RestorePlan, SavedState, SkipReason, TargetValue, TurnEnd, TurnId,
    TurnState, Value, WireRefusal, Workspace,
};
use docket_dbus::CompanionTerminalsProxy;
use porter_core::AppName;
use prov::{ActionName, Label, Labelled, SessionId};
use temor_ssh::UnixSeconds;
use tokio::runtime::Handle;

/// The app that declares the restore.
const APP: &str = "org.quire.Checkpoints";
/// The restore action.
const RESTORE: &str = "checkpoints.restore";

/// The restore points over the session bus. Calls block the thread they are made on, so they
/// belong on a worker thread, never on the window's.
#[derive(Debug)]
pub struct SystemCheckpoints {
    intents: Intents<DbusTransport>,
    /// The assistant's terminals, which recognise an agent by its command line; `None` when its
    /// proxy could not be made (the watch is then the plain one).
    terminals: Option<CompanionTerminalsProxy<'static>>,
    runtime: Handle,
}

impl SystemCheckpoints {
    /// A reader on the session bus of `env`; calls wait on `runtime`, which the caller keeps alive.
    pub async fn connect(env: &DesktopEnv, runtime: Handle) -> Result<Self, DesktopError> {
        let connection = connect(env).await?;
        let terminals = CompanionTerminalsProxy::new(&connection).await.ok();
        Ok(Self {
            intents: Intents::over(DbusTransport::new(connection)),
            terminals,
            runtime,
        })
    }
}

impl CheckpointPort for SystemCheckpoints {
    fn list(&self, session: &SessionKey) -> Result<PointList, DesktopError> {
        let id = session_id(session)?;
        let list = self
            .runtime
            .block_on(self.intents.checkpoint_list(id))
            .map_err(|error| client_fault("read the restore points", &error))?;
        let mut points = list_of(&list.rows);
        points.turn = run_state_of(list.turn);
        Ok(points)
    }

    fn plan(&self, session: &SessionKey, id: PointId) -> Result<PointPlan, DesktopError> {
        let session = session_id(session)?;
        let plan = self
            .runtime
            .block_on(self.intents.checkpoint_plan(session, CheckpointId(id.0)))
            .map_err(|error| client_fault("read what a restore would change", &error))?;
        plan.map(|plan| plan_of(&plan)).map_err(fault_of)
    }

    fn restore(
        &self,
        session: &SessionKey,
        id: PointId,
        digest: &PlanDigest,
    ) -> Result<(), DesktopError> {
        let call = restore_call(session, id, digest)?;
        let end = self
            .runtime
            .block_on(self.intents.perform(call, None, None))
            .map_err(|error| client_fault("restore the files", &error))?;
        end.map(drop).map_err(refusal_fault)
    }

    fn watch(&self, ask: &WatchAsk) -> Result<Watching, DesktopError> {
        let workspace = Workspace::parse(&ask.workspace).map_err(|_| failed())?;
        let rewind = match ask.rewind {
            Rewind::Docket => docket_core::Rewind::Docket,
            Rewind::Agent => docket_core::Rewind::Agent,
        };
        let label = ask.label.clone();
        if let Some(agent) = self.recognise(ask) {
            let watching = self
                .runtime
                .block_on(
                    self.intents
                        .checkpoint_watch_agent(workspace, label, rewind, agent),
                )
                .map_err(|error| client_fault("watch the agent", &error))?;
            let owner = match watching.owner {
                docket_core::WatchOwner::Companion => WatchOwner::Companion,
                _ => WatchOwner::Terminal,
            };
            let session = SessionKey::new(watching.session.as_str()).ok_or_else(failed)?;
            return Ok(Watching {
                session,
                owner,
                named: Named::App,
            });
        }
        let id = self
            .runtime
            .block_on(self.intents.checkpoint_watch(workspace, label, rewind))
            .map_err(|error| client_fault("watch the agent", &error))?;
        SessionKey::new(id.as_str())
            .map(Watching::plain)
            .ok_or_else(failed)
    }

    fn mark(&self, session: &SessionKey) -> Result<TurnMark, DesktopError> {
        let id = session_id(session)?;
        let turn = self
            .runtime
            .block_on(self.intents.checkpoint_mark(id))
            .map_err(|error| client_fault("save a restore point", &error))?;
        Ok(TurnMark(turn.0))
    }

    fn turn_ended(
        &self,
        session: &SessionKey,
        turn: TurnMark,
        how: TurnHow,
    ) -> Result<(), DesktopError> {
        let id = session_id(session)?;
        let how = match how {
            TurnHow::Answered => TurnEnd::Answered,
            TurnHow::Failed => TurnEnd::Failed,
            TurnHow::Cancelled => TurnEnd::Cancelled,
        };
        self.runtime
            .block_on(self.intents.session_turn_ended(id, TurnId(turn.0), how))
            .map_err(|error| client_fault("end the turn", &error))
    }

    fn close(&self, session: &SessionKey) -> Result<(), DesktopError> {
        let id = session_id(session)?;
        self.runtime
            .block_on(self.intents.session_close(id))
            .map_err(|error| client_fault("stop watching the agent", &error))
    }
}

impl SystemCheckpoints {
    /// The configured app and conversation the assistant recognises in `ask`'s command line, if
    /// it does. Any trouble (no command line, no assistant, a failed call, an answer this build
    /// cannot read) is "not recognised", and the watch is the plain one.
    fn recognise(&self, ask: &WatchAsk) -> Option<docket_core::WatchedAgent> {
        let terminals = self.terminals.as_ref()?;
        let seen = docket_core::SeenCommand::parse(ask.seen.as_ref()?.tokens().to_vec()).ok()?;
        let body = serde_json::to_string(&seen).ok()?;
        let answer = self.runtime.block_on(terminals.recognise(&body)).ok()?;
        serde_json::from_str::<Option<docket_core::WatchedAgent>>(&answer)
            .ok()
            .flatten()
    }
}

/// docket's turn state as the window's.
pub(super) fn run_state_of(state: TurnState) -> RunState {
    match state {
        TurnState::Running => RunState::Running,
        _ => RunState::Idle,
    }
}

fn failed() -> DesktopError {
    DesktopError::Checkpoint(PointFault::Failed)
}

fn session_id(session: &SessionKey) -> Result<SessionId, DesktopError> {
    SessionId::parse(session.as_str())
        .map_err(|_| DesktopError::Checkpoint(PointFault::NoSuchPoint))
}

/// A named argument the person's own click gave.
fn arg(name: &str, value: Value) -> Result<(ParamName, Labelled<Value>), DesktopError> {
    let name = ParamName::parse(name).map_err(|_| failed())?;
    let label = Label::trusted_user();
    Ok((name, Labelled { value, label }))
}

/// The call for `checkpoints.restore`: session, point and the plan's digest.
pub(super) fn restore_call(
    session: &SessionKey,
    id: PointId,
    digest: &PlanDigest,
) -> Result<CallRequest, DesktopError> {
    let args: Args = [
        arg("session", Value::Text(session.as_str().to_owned()))?,
        arg("point", Value::Integer(i64::from(id.0)))?,
        arg("plan", Value::Text(digest.0.clone()))?,
    ]
    .into_iter()
    .collect();
    Ok(CallRequest {
        action: ActionRef {
            app: AppName::parse(APP).map_err(|_| failed())?,
            name: ActionName::parse(RESTORE).map_err(|_| failed())?,
        },
        target: TargetValue::Nothing,
        args,
        // docket lists temor under its `cli` role, and the cli role's calls come from `Origin::Cli`
        // (docket-cli and docket's own eval pair them); the router derives the arguments' source
        // from the role, and policy keys on the origin only for the launcher's.
        origin: Origin::Cli,
    })
}

fn seconds(at: porter_core::UnixSeconds) -> UnixSeconds {
    UnixSeconds(u64::try_from(at.0).unwrap_or(0))
}

fn why_of(why: &SkipReason) -> NotSavedWhy {
    match why {
        SkipReason::AgentKeepsOwn => NotSavedWhy::AgentKeepsOwn,
        SkipReason::NoHistory => NotSavedWhy::NoHistory,
        SkipReason::TooLarge => NotSavedWhy::TooLarge,
        SkipReason::Failed => NotSavedWhy::Failed,
        _ => NotSavedWhy::Unknown,
    }
}

/// docket's list as the window's plain one; a row this build does not know is left out.
pub(super) fn list_of(rows: &[CheckpointRow]) -> PointList {
    let rows = rows
        .iter()
        .filter_map(|row| match row {
            CheckpointRow::Saved {
                id,
                turn,
                at,
                state,
            } => Some(PointRow::Saved {
                id: PointId(id.0),
                turn: TurnNo(turn.0),
                at: seconds(*at),
                state: match state {
                    SavedState::Available => PointState::Available,
                    SavedState::Gone => PointState::Gone,
                },
            }),
            CheckpointRow::NotSaved { turn, at, why } => Some(PointRow::NotSaved {
                turn: TurnNo(turn.0),
                at: seconds(*at),
                why: why_of(why),
            }),
            CheckpointRow::RestoredTo { id, at } => Some(PointRow::RestoredTo {
                id: PointId(id.0),
                at: seconds(*at),
            }),
            _ => None,
        })
        .collect();
    PointList {
        rows,
        turn: RunState::Idle,
    }
}

pub(super) fn plan_of(plan: &RestorePlan) -> PointPlan {
    let names = |paths: &[docket_core::WorkPath]| -> Vec<String> {
        paths.iter().map(|p| p.as_str().to_owned()).collect()
    };
    PointPlan {
        changed: names(&plan.changed),
        added: names(&plan.added),
        removed: names(&plan.removed),
        digest: PlanDigest(plan.digest.0.clone()),
    }
}

/// docket's fault as the window's.
pub(super) fn fault_of(fault: CheckpointFault) -> DesktopError {
    let fault = match fault {
        CheckpointFault::NoSuchPoint => PointFault::NoSuchPoint,
        CheckpointFault::Gone => PointFault::Gone,
        CheckpointFault::TurnRunning => PointFault::TurnRunning,
        CheckpointFault::PlanStale => PointFault::PlanStale,
        CheckpointFault::NoHistory => PointFault::NoHistory,
        _ => PointFault::Failed,
    };
    DesktopError::Checkpoint(fault)
}

/// Why a performed call did not run.
pub(super) fn refusal_fault(refusal: CallRefusal) -> DesktopError {
    let fault = match refusal {
        CallRefusal::Denied(_) => PointFault::Denied,
        CallRefusal::Unconfirmed(_) => PointFault::Declined,
        _ => PointFault::Failed,
    };
    DesktopError::Checkpoint(fault)
}

/// A request that did not give its reply: not allowed is `Denied`, anything else is the bus's.
fn client_fault(call: &'static str, error: &ClientError) -> DesktopError {
    match error {
        ClientError::Refused(WireRefusal::NotAllowed) => {
            DesktopError::Checkpoint(PointFault::Denied)
        }
        other => DesktopError::bus(call, other),
    }
}
