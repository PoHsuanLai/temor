use temor_pane::CommandLine;

/// Programs temor knows as agents by their process name (lowercase, exact). The weakest evidence:
/// a program's own report (OSC 7501 or the pane socket) counts as well.
const KNOWN_AGENTS: &[&str] = &[
    "claude",
    "codex",
    "gemini",
    "agy",
    "aider",
    "opencode",
    "goose",
    "amp",
    "crush",
    "cursor-agent",
];

/// A known agent program's name, lowercase.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AgentName(String);

impl AgentName {
    /// `name` as an agent name, when it is a known agent program (any case, otherwise exact).
    pub fn known(name: &str) -> Option<AgentName> {
        let name = name.to_ascii_lowercase();
        KNOWN_AGENTS
            .contains(&name.as_str())
            .then_some(AgentName(name))
    }

    /// The lowercase program name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Whether `name` (a process name or a report's app) is a known agent program.
pub fn is_known_agent(name: &str) -> bool {
    AgentName::known(name).is_some()
}

/// The known agent a foreground process is: by its process name `comm`, else by the file name of
/// any argument of its command line (so `bwrap -- claude --resume x` is `claude`).
pub fn agent_of(comm: &str, command: Option<&CommandLine>) -> Option<AgentName> {
    AgentName::known(comm).or_else(|| {
        command?.tokens().iter().find_map(|token| {
            let file = token.rsplit('/').next().unwrap_or(token);
            AgentName::known(file)
        })
    })
}
