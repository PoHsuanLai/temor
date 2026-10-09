//! What the overlap check finds (D19, spike `spikes/worktree/SPIKE.md`): two severities only.

use crate::vocab::ids::TabId;
use crate::vocab::text::RelPath;
use serde::{Deserialize, Serialize};

/// What the agent's work is compared with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Other {
    /// The base branch.
    Base,
    /// Another agent's tab.
    Tab(TabId),
}

/// How loud a finding is. There is nothing above red and nothing between.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Severity {
    /// Worth a look: both touch the same files.
    Amber,
    /// The two cannot merge cleanly.
    Red,
}

/// One thing the check found about an agent's worktree.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Finding {
    /// A merge with `with` would conflict in `file`. Red.
    Conflicts {
        /// Who it conflicts with.
        with: Other,
        /// Where.
        file: RelPath,
    },
    /// Both changed `file` (no conflict yet, or a rename against an edit). Amber.
    BothTouch {
        /// Who else.
        with: Other,
        /// The file.
        file: RelPath,
    },
    /// The base has moved since the worktree was cut. Amber.
    BaseMoved {
        /// Files the base changed.
        files: usize,
        /// Of those, files the agent changed too.
        overlap: usize,
    },
}

impl Finding {
    /// Red for a conflict, amber for the rest.
    pub fn severity(&self) -> Severity {
        match self {
            Finding::Conflicts { .. } => Severity::Red,
            Finding::BothTouch { .. } | Finding::BaseMoved { .. } => Severity::Amber,
        }
    }

    /// The line the tab shows. `name` gives a tab's title (and the base's name) for the person.
    pub fn text(&self, name: impl Fn(Other) -> String) -> String {
        match self {
            Finding::Conflicts { with, file } => {
                format!("Conflicts with {} in {}", name(*with), file.as_str())
            }
            Finding::BothTouch { file, .. } => format!("Both touch {}", file.as_str()),
            Finding::BaseMoved { files, overlap } => {
                let noun = if *files == 1 { "file" } else { "files" };
                format!("Base moved: {files} {noun}, {overlap} overlap yours")
            }
        }
    }
}

/// The note shown next to the findings: the check reads text, not meaning.
pub const TEXT_OVERLAP_ONLY: &str = "Text overlap only";

/// Plain facts about an agent's worktree against its base.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Divergence {
    /// Commits the agent has that the base does not.
    pub ahead: u32,
    /// Commits the base has that the agent does not.
    pub behind: u32,
    /// Files the agent changed since the base.
    pub files_changed: u32,
}

/// Everything the tab shows about one agent: red first, then amber, then the plain facts.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AgentOverlap {
    /// Findings, red before amber.
    pub findings: Vec<Finding>,
    /// Ahead, behind and files changed, once the base has been probed.
    pub divergence: Option<Divergence>,
}
