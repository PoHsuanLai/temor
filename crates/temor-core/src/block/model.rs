//! A command block (D5, from OSC 133 A/B/C/D) and who wrote its command.

use crate::vocab::ids::{BlockId, ExitCode, GridSpan, TurnRef};
use crate::vocab::text::{AgentLabel, CommandText};
use ds_core::time::stamp::Stamp;
use ds_core::word::Word;
use serde::{Deserialize, Serialize};

/// One prompt, the command typed at it, and what the command printed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    /// Its identity.
    pub id: BlockId,
    /// The prompt's rows.
    pub prompt: GridSpan,
    /// The command line.
    pub command: CommandText,
    /// The output's rows.
    pub output: GridSpan,
    /// How it exited; none when the shell did not say.
    #[serde(default)]
    pub exit: Option<ExitCode>,
    /// Who wrote the command.
    pub by: Author,
    /// When it started running.
    pub started: Stamp,
    /// When it finished; none while it runs.
    #[serde(default)]
    pub ended: Option<Stamp>,
}

/// Who wrote a block's command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum Author {
    /// The person typed it.
    Person,
    /// The person ran a proposal they asked for (§8.3).
    Proposal {
        /// The model turn that suggested it.
        turn: TurnRef,
        /// Whether that turn read text from outside.
        taint: Taint,
    },
    /// An agent ran it.
    Agent(AgentLabel),
}

/// Whether a model turn read text it cannot vouch for (command output, a web page), which the
/// proposal chip marks as "suggested from the output above".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum Taint {
    /// The turn read only what the person wrote.
    Clean,
    /// The turn read untrusted text.
    ReadUntrusted,
}
