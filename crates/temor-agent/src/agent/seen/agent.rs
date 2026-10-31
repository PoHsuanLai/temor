use temor_schema::process::CommandLine;

use super::{node, wrapper};

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

/// A known agent found in a foreground process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sighting {
    pub agent: AgentName,
    /// The index of the program word in the command line (past any `env` or `bwrap` in front of
    /// it); `0` when the agent was known by its process name.
    pub program_at: usize,
}

/// The known agent a foreground process is: by its process name `comm`, else by the program word
/// of its command line, found through wrappers (so `bwrap -- claude --resume x` is `claude`, and
/// `rg claude` is not). A program `node` is the agent its script is: the script's file name
/// (without extension) is a known agent, or its path has a `claude-code` directory (the
/// Anthropic package's `cli.js`, which is `claude`).
pub fn agent_of(comm: &str, command: Option<&CommandLine>) -> Option<Sighting> {
    if let Some(agent) = AgentName::known(comm) {
        return Some(Sighting {
            agent,
            program_at: 0,
        });
    }
    let tokens = command?.tokens();
    let program_at = wrapper::program_at(tokens)?;
    let file = wrapper::file_name(tokens.get(program_at)?);
    let agent = AgentName::known(file).or_else(|| {
        let script = node::node_script(tokens, program_at).filter(|_| file == "node")?;
        script_agent(script)
    })?;
    Some(Sighting { agent, program_at })
}

/// The agent a `node` script is.
fn script_agent(script: &str) -> Option<AgentName> {
    let file = wrapper::file_name(script);
    let stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
    AgentName::known(stem).or_else(|| {
        script
            .split('/')
            .any(|part| part == "claude-code")
            .then(|| AgentName::known("claude"))
            .flatten()
    })
}
