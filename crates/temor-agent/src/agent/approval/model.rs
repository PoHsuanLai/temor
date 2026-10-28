//! The two facts of the mode line and where they come from.

use ds_core::word::Word;
use serde::{Deserialize, Serialize};

/// Whether the agent's process runs in the sandbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum SandboxMode {
    /// Inside the sandbox.
    #[word(label = "On")]
    On,
    /// Not sandboxed.
    #[word(label = "Off")]
    Off,
}

/// What the gate asks the person before it lets an action through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum AsksLevel {
    /// Every action.
    #[word(label = "Everything")]
    Everything,
    /// Writes and anything that changes the machine; reads go through.
    #[word(label = "Writes")]
    Writes,
    /// Nothing: the gate lets actions through. Only the sandbox limits the agent.
    #[word(label = "Nothing")]
    Nothing,
}

/// The two facts together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ApprovalMode {
    /// Sandbox on or off.
    pub sandbox: SandboxMode,
    /// What it asks.
    pub asks: AsksLevel,
}

/// What the launch profile says about the process (docket-launch's bubblewrap, or none).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchFact {
    /// Started through docket-launch's sandbox.
    Sandboxed,
    /// Started plain.
    Unsandboxed,
    /// The profile could not be read.
    Unknown,
}

/// What the gate's session for this agent says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateFact {
    /// There is a session, asking at this level.
    Session(AsksLevel),
    /// No session: the agent's tool calls do not pass the gate (an unbridged CLI).
    NoSession,
}

/// What the header's mode line shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApprovalLine {
    /// Both facts are known.
    Gated(ApprovalMode),
    /// temor cannot read what the agent is asked. The sandbox is shown when it is known.
    NotGated {
        /// The sandbox, when the launch profile said.
        sandbox: Option<SandboxMode>,
    },
}

/// The line for the two facts. Both must be known to be shown as gated.
pub fn approval_line(launch: LaunchFact, gate: GateFact) -> ApprovalLine {
    let sandbox = match launch {
        LaunchFact::Sandboxed => Some(SandboxMode::On),
        LaunchFact::Unsandboxed => Some(SandboxMode::Off),
        LaunchFact::Unknown => None,
    };
    match (sandbox, gate) {
        (Some(sandbox), GateFact::Session(asks)) => {
            ApprovalLine::Gated(ApprovalMode { sandbox, asks })
        }
        (sandbox, _) => ApprovalLine::NotGated { sandbox },
    }
}

impl ApprovalLine {
    /// The text on the mode line: `Sandbox: On · Asks: Writes`, or `Not gated`.
    pub fn text(&self) -> String {
        match self {
            ApprovalLine::Gated(m) => {
                format!("Sandbox: {} · Asks: {}", m.sandbox.label(), m.asks.label())
            }
            ApprovalLine::NotGated { .. } => "Not gated".to_owned(),
        }
    }
}

/// The tab's tooltip and radar row detail: the same two facts, spelled out.
pub fn tooltip(line: &ApprovalLine) -> String {
    match line {
        ApprovalLine::Gated(m) => {
            let sandbox = match m.sandbox {
                SandboxMode::On => "runs in a sandbox",
                SandboxMode::Off => "runs without a sandbox",
            };
            let asks = match m.asks {
                AsksLevel::Everything => "asks before every action",
                AsksLevel::Writes => "asks before it changes anything",
                AsksLevel::Nothing => "asks about nothing",
            };
            format!("This agent {sandbox} and {asks}")
        }
        ApprovalLine::NotGated {
            sandbox: Some(SandboxMode::On),
        } => "This agent runs in a sandbox, but temor cannot see what it is asked".to_owned(),
        ApprovalLine::NotGated { .. } => "temor cannot see what this agent is asked".to_owned(),
    }
}

/// A change the person can make on one pane, from the menu on its mode line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModeChange {
    /// Ask at another level.
    Asks(AsksLevel),
    /// Turn the sandbox on or off.
    Sandbox(SandboxMode),
}

impl ModeChange {
    /// The consequence spelled out next to the choice.
    pub fn consequence(&self) -> &'static str {
        match self {
            ModeChange::Asks(AsksLevel::Everything) => "Every action waits for your answer",
            ModeChange::Asks(AsksLevel::Writes) => {
                "Reading goes through; anything that changes files or runs commands waits for you"
            }
            ModeChange::Asks(AsksLevel::Nothing) => {
                "Actions go through without asking; only the sandbox limits this agent"
            }
            ModeChange::Sandbox(SandboxMode::On) => "Restarts this agent inside the sandbox",
            ModeChange::Sandbox(SandboxMode::Off) => {
                "Restarts this agent with full access to your files"
            }
        }
    }

    /// Whether the agent has to be restarted for it to take hold.
    pub fn restarts_agent(&self) -> bool {
        matches!(self, ModeChange::Sandbox(_))
    }
}
