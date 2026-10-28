//! What a pane is: a terminal (a shell, or an agent's own program), an agent view, or a viewer.

use crate::vocab::ids::AgentSessionRef;
use crate::vocab::text::{AgentLabel, Cwd, HostAlias, ViewerRef};
use ds_core::word::Word;
use serde::{Deserialize, Serialize};

/// Whether a pane has the keyboard in the window the person is using. A notice goes out only
/// while it does not, and an agent's finished turn is seen at once while it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
#[word(case = snake)]
pub enum PaneFocus {
    Focused,
    Unfocused,
}

/// The kind of thing a pane shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum PaneKind {
    /// A terminal grid.
    Terminal(TerminalPane),
    /// The gated agent view (D8), driving a session through docket.
    AgentView(AgentSessionRef),
    /// A file shown by the viewer.
    Viewer(ViewerRef),
}

/// A terminal pane: what runs in it and where.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalPane {
    /// What the pane runs.
    pub role: TerminalRole,
    /// The folder it was last in (OSC 7).
    #[serde(default)]
    pub cwd: Option<Cwd>,
    /// The host it runs on; none for this computer.
    #[serde(default)]
    pub host: Option<HostAlias>,
}

/// What a terminal pane runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum TerminalRole {
    /// The person's shell.
    Shell,
    /// An agent's own program, opted into (D8): never ungated.
    AgentCli {
        /// Which agent.
        program: AgentLabel,
        /// How its actions are held.
        gate: CliGate,
    },
}

/// How an agent program in a terminal pane is held. There is no "free" member: both are shown
/// to the person by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum CliGate {
    /// Each tool call is put to the desktop's gate through the agent's own pre-tool hook.
    #[word(label = "Checked")]
    HookBridged,
    /// It runs in a sandbox and nothing checks each action.
    #[word(label = "Ungated, sandboxed")]
    Sandboxed,
}
