//! temor's four capabilities and what a manifest's `[[provides]]` entry says about each (PLAN D14).
//!
//! ```toml
//! [[provides]]
//! capability = "block.lens"
//! trigger = "kubectl get"        # the commands whose blocks it draws
//!
//! [[provides]]
//! capability = "open.provider"
//! trigger = "gh"                 # the word that sends a query here; absent for every query
//!
//! [[provides]]
//! capability = "agent.adapter"
//! agent = "claude"               # the agent it reports on
//!
//! [[provides]]
//! capability = "row.facts"
//! tables = ["folder"]            # the rows it knows; temor asks for "folder"
//! ```

use bayonet::manifest::{Entry, Provides, Refusal};
use serde::{Deserialize, Serialize};

use crate::trigger::Trigger;

/// The four things a plugin of temor can offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Cap {
    /// Draws an image for a block.
    #[serde(rename = "block.lens")]
    BlockLens,
    /// Answers what is typed in the command bar with rows.
    #[serde(rename = "open.provider")]
    OpenProvider,
    /// Reports what an agent is doing.
    #[serde(rename = "agent.adapter")]
    AgentAdapter,
    /// Adds facts about a folder to the chips and the sidebar row.
    #[serde(rename = "row.facts")]
    RowFacts,
}

impl Cap {
    /// The name a manifest spells it with.
    pub fn name(self) -> &'static str {
        match self {
            Cap::BlockLens => "block.lens",
            Cap::OpenProvider => "open.provider",
            Cap::AgentAdapter => "agent.adapter",
            Cap::RowFacts => "row.facts",
        }
    }
}

/// The table `row.facts` is asked about for a pane's folder.
pub const FOLDER_TABLE: &str = "folder";

/// One `[[provides]]` entry: the capability and what it applies to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provision {
    /// The commands whose blocks it draws.
    BlockLens(Trigger),
    /// The word that routes a query to it; empty for every query.
    OpenProvider(Trigger),
    /// The agent it reports on.
    AgentAdapter(String),
    /// The tables whose rows it knows.
    RowFacts(Vec<String>),
}

/// Why temor refuses an entry that is well formed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Fault {
    /// A lens, an adapter or a facts entry that applies to nothing.
    #[error("{} applies to nothing: it needs {needs}", .capability.name())]
    AppliesToNothing {
        /// Which capability.
        capability: Cap,
        /// The key it lacks.
        needs: &'static str,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    capability: Cap,
    #[serde(default)]
    trigger: String,
    #[serde(default)]
    agent: String,
    #[serde(default)]
    tables: Vec<String>,
}

fn nothing(capability: Cap, needs: &'static str) -> Refusal<Fault> {
    Refusal::Fault(Fault::AppliesToNothing { capability, needs })
}

impl Provides for Provision {
    type Capability = Cap;
    type Fault = Fault;

    fn parse(entry: Entry) -> Result<Provision, Refusal<Fault>> {
        let raw: Raw = entry.read()?;
        let trigger = Trigger::new(&raw.trigger);
        match raw.capability {
            Cap::BlockLens if trigger.is_empty() => Err(nothing(Cap::BlockLens, "`trigger`")),
            Cap::BlockLens => Ok(Provision::BlockLens(trigger)),
            Cap::OpenProvider => Ok(Provision::OpenProvider(trigger)),
            Cap::AgentAdapter if raw.agent.trim().is_empty() => {
                Err(nothing(Cap::AgentAdapter, "`agent`"))
            }
            Cap::AgentAdapter => Ok(Provision::AgentAdapter(raw.agent.trim().to_owned())),
            Cap::RowFacts if raw.tables.is_empty() => Err(nothing(Cap::RowFacts, "`tables`")),
            Cap::RowFacts => Ok(Provision::RowFacts(raw.tables)),
        }
    }

    fn capability(&self) -> Cap {
        match self {
            Provision::BlockLens(_) => Cap::BlockLens,
            Provision::OpenProvider(_) => Cap::OpenProvider,
            Provision::AgentAdapter(_) => Cap::AgentAdapter,
            Provision::RowFacts(_) => Cap::RowFacts,
        }
    }
}
