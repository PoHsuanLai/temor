//! docket's restore points over the session bus: the list and the plan are `Checkpoint.List` and
//! `Checkpoint.Plan`; the restore is the destructive action `checkpoints.restore` of
//! `org.quire.Checkpoints`, performed with the digest of the plan the person saw. docket draws
//! the confirmation where its policy says (the action asks every time).

use super::bus::connect;
use crate::checkpoints::{
    CheckpointPort, NotSavedWhy, PlanDigest, PointFault, PointId, PointList, PointPlan, PointRow,
    PointState, SessionKey, TurnNo,
};
use crate::{DesktopEnv, DesktopError};
use docket_client::{ClientError, DbusTransport, Intents};
use docket_core::{
    ActionRef, Args, CallRefusal, CallRequest, CheckpointFault, CheckpointId, CheckpointRow,
    Origin, ParamName, RestorePlan, SavedState, SkipReason, TargetValue, Value, WireRefusal,
};
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
    runtime: Handle,
}

impl SystemCheckpoints {
    /// A reader on the session bus of `env`; calls wait on `runtime`, which the caller keeps alive.
    pub async fn connect(env: &DesktopEnv, runtime: Handle) -> Result<Self, DesktopError> {
        let connection = connect(env).await?;
        Ok(Self {
            intents: Intents::over(DbusTransport::new(connection)),
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
        Ok(list_of(&list.rows))
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
        origin: Origin::InWindowField,
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
    PointList { rows }
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
