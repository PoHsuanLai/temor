//! What a watch of an agent is asked and answered (PLAN P19, docket DESIGN §12): who keeps the
//! history, which command line the agent was started with, who closes the session, and the turns
//! marked on it.

use crate::SessionKey;
use temor_pty::CommandLine;

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
    /// What the agent's process was started with, from its program word on (never the wrapper
    /// words in front of it, which can hold secrets), when it could be read: docket uses it to
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
