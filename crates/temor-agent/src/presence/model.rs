//! An agent's state in a pane (§8.1), and the reports that move it.

use ds_core::word::Word;
use serde::{Deserialize, Serialize};
use temor_core::vocab::ids::{PaneId, Seq};
use temor_core::vocab::text::{Note, PluginId};

/// What the agent in a pane is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum AgentState {
    /// Nothing under way.
    #[default]
    Idle,
    /// Working.
    Working,
    /// Blocked on the person.
    Waiting(WaitKind),
    /// Finished; stays until the pane is seen.
    Done(Seen),
    /// Failed; stays until the pane is seen.
    Failed(Seen),
}

/// What a waiting agent needs from the person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum WaitKind {
    /// To allow an action.
    Permission,
    /// An answer.
    Question,
    /// To sign in.
    SignIn,
}

/// Whether the person has looked at a finished pane since it finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum Seen {
    /// Not yet: the row keeps its unread dot.
    Unseen,
    /// Looked at.
    Seen,
}

/// One report of an agent's state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusReport {
    /// The pane it is about.
    pub pane: PaneId,
    /// The state it reports.
    pub state: AgentState,
    /// Its place in its source's sequence; an older one than already seen is ignored.
    pub seq: Seq,
    /// Who reported it.
    pub source: StatusSource,
    /// The agent's own words, carried into a notification.
    #[serde(default)]
    pub note: Option<Note>,
}

/// Where a report came from, strongest first.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum StatusSource {
    /// The program's own status sequence (OSC 7501).
    ProgramStatus,
    /// An `agent.adapter` plugin over temor's socket.
    Adapter(PluginId),
    /// docket's companion roster, for agent-view sessions.
    Docket,
    /// A notification sequence or a bell.
    Notification,
}
