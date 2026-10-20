//! A session's restore points, as the window reads them (PLAN P19). docket keeps the points and
//! does the restore; the window asks this port for the list, for what a restore would change, and
//! for the restore itself. The types here are plain: nothing of docket's crosses into the window.
//!
//! The port is blocking, because the window calls it from a worker thread of its own (as it runs
//! git); the live one waits on the bus task's runtime.

use std::fmt;

use temor_pty::CommandLine;
use temor_ssh::UnixSeconds;

use crate::DesktopError;

/// A docket session, as docket numbers it: the key of one run's restore points.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionKey(String);

impl SessionKey {
    /// `text` as a session key: 1 to 128 bytes, no control characters. `None` otherwise.
    pub fn new(text: &str) -> Option<SessionKey> {
        let ok = !text.is_empty() && text.len() <= 128 && !text.chars().any(char::is_control);
        ok.then(|| SessionKey(text.to_owned()))
    }

    /// The key's text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A restore point's number within one session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PointId(pub u32);

/// A turn's number within one session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TurnNo(pub u64);

/// Binds a restore to exactly the plan the person saw; given back as it was received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanDigest(pub String);

/// Whether a saved point can still be restored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointState {
    Available,
    /// Cleared since (kept for 20 points and 14 days).
    Gone,
}

/// Why a turn has no restore point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotSavedWhy {
    /// The agent keeps its own history.
    AgentKeepsOwn,
    /// The folder is not in git, so docket skipped it.
    NoHistory,
    /// The folder holds too many files.
    TooLarge,
    /// Saving went wrong or took too long.
    Failed,
    /// A reason this build does not know.
    Unknown,
}

/// One row of the list, oldest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointRow {
    Saved {
        id: PointId,
        turn: TurnNo,
        at: UnixSeconds,
        state: PointState,
    },
    NotSaved {
        turn: TurnNo,
        at: UnixSeconds,
        why: NotSavedWhy,
    },
    RestoredTo {
        id: PointId,
        at: UnixSeconds,
    },
}

/// Whether the session's agent is working now.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RunState {
    #[default]
    Idle,
    /// A turn is running: docket refuses a restore until it ends.
    Running,
}

/// A session's list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PointList {
    pub rows: Vec<PointRow>,
    /// Whether a turn is running now.
    pub turn: RunState,
}

/// Who keeps the history of a watched agent's file changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rewind {
    /// docket saves a restore point at each turn.
    Docket,
    /// The agent keeps its own: docket saves nothing and says so.
    Agent,
}

/// What a watch is asked for: an agent running in `workspace`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchAsk {
    /// An absolute folder.
    pub workspace: String,
    /// Display text.
    pub label: String,
    pub rewind: Rewind,
    /// What the agent's process was started with, when it could be read: docket uses it to
    /// recognise the configured app and the conversation.
    pub seen: Option<CommandLine>,
}

/// Who closes a watch's session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchOwner {
    /// The window's: it closes the session when the agent or its pane goes.
    Terminal,
    /// The assistant's: the session was already there (a window it handed to the terminal). The
    /// window marks turns on it and never closes it.
    Companion,
}

/// Whether docket recognised the watched agent's app and conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Named {
    /// Recognised: the session belongs to that conversation, and outlives one run of the agent.
    App,
    /// Not recognised: the session only keeps restore points for this run.
    Unknown,
}

/// A watch docket opened, or found already open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watching {
    pub session: SessionKey,
    pub owner: WatchOwner,
    pub named: Named,
}

impl Watching {
    /// A session the window opened for an agent docket did not recognise.
    pub fn plain(session: SessionKey) -> Self {
        Self {
            session,
            owner: WatchOwner::Terminal,
            named: Named::Unknown,
        }
    }
}

/// A turn of a watched agent, as `mark` numbered it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TurnMark(pub u64);

/// How a watched agent's turn ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnHow {
    Answered,
    Failed,
    Cancelled,
}

/// What restoring one point would change: paths relative to the folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PointPlan {
    /// Files put back to their older content.
    pub changed: Vec<String>,
    /// Files brought back.
    pub added: Vec<String>,
    /// Files deleted (made since).
    pub removed: Vec<String>,
    pub digest: PlanDigest,
}

/// Why docket would not list, plan or restore.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointFault {
    NoSuchPoint,
    Gone,
    TurnRunning,
    /// The folder changed since the plan was shown.
    PlanStale,
    NoHistory,
    Failed,
    /// The desktop's agent layer did not allow it.
    Denied,
    /// The person said no.
    Declined,
}

impl PointFault {
    /// The words for the person.
    pub fn words(self) -> &'static str {
        match self {
            PointFault::NoSuchPoint => "There is no such restore point",
            PointFault::Gone => "That restore point has been cleared",
            PointFault::TurnRunning => "Wait for the agent to finish",
            PointFault::PlanStale => "The folder changed since you looked; look again",
            PointFault::NoHistory => "This folder cannot keep restore points",
            PointFault::Failed => "The restore could not be completed",
            PointFault::Denied => "The desktop did not allow the restore",
            PointFault::Declined => "The restore was cancelled",
        }
    }
}

impl fmt::Display for PointFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.words())
    }
}

/// The window's view of docket's restore points. `Send + Sync`: calls come from worker threads.
pub trait CheckpointPort: Send + Sync + fmt::Debug {
    /// The session's points, oldest first.
    fn list(&self, session: &SessionKey) -> Result<PointList, DesktopError>;

    /// What restoring `id` would change. Read-only.
    fn plan(&self, session: &SessionKey, id: PointId) -> Result<PointPlan, DesktopError>;

    /// Puts the folder back to `id`, bound to the plan `digest` the person saw. docket asks the
    /// person again where its policy says to.
    fn restore(
        &self,
        session: &SessionKey,
        id: PointId,
        digest: &PlanDigest,
    ) -> Result<(), DesktopError>;

    /// Starts a session that only keeps restore points for the agent `ask` describes, or finds
    /// the one docket already has for it.
    fn watch(&self, ask: &WatchAsk) -> Result<Watching, DesktopError>;

    /// The watched agent started working: a point is taken (unless it keeps its own) and a turn
    /// begins. End it with [`turn_ended`](Self::turn_ended).
    fn mark(&self, session: &SessionKey) -> Result<TurnMark, DesktopError>;

    /// The turn `mark` numbered is over.
    fn turn_ended(
        &self,
        session: &SessionKey,
        turn: TurnMark,
        how: TurnHow,
    ) -> Result<(), DesktopError>;

    /// The watched agent is gone.
    fn close(&self, session: &SessionKey) -> Result<(), DesktopError>;
}

/// No agent layer: nothing is listed and nothing can be restored. The default.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoCheckpoints;

impl CheckpointPort for NoCheckpoints {
    fn list(&self, _session: &SessionKey) -> Result<PointList, DesktopError> {
        Ok(PointList::default())
    }

    fn plan(&self, _session: &SessionKey, _id: PointId) -> Result<PointPlan, DesktopError> {
        Err(DesktopError::Unsupported {
            what: "restore points",
        })
    }

    fn restore(
        &self,
        _session: &SessionKey,
        _id: PointId,
        _digest: &PlanDigest,
    ) -> Result<(), DesktopError> {
        Err(DesktopError::Unsupported {
            what: "restore points",
        })
    }

    fn watch(&self, _ask: &WatchAsk) -> Result<Watching, DesktopError> {
        Err(DesktopError::Unsupported {
            what: "restore points",
        })
    }

    fn mark(&self, _session: &SessionKey) -> Result<TurnMark, DesktopError> {
        Err(DesktopError::Unsupported {
            what: "restore points",
        })
    }

    fn turn_ended(
        &self,
        _session: &SessionKey,
        _turn: TurnMark,
        _how: TurnHow,
    ) -> Result<(), DesktopError> {
        Ok(())
    }

    fn close(&self, _session: &SessionKey) -> Result<(), DesktopError> {
        Ok(())
    }
}
