//! A command block (D5, from OSC 133 A/B/C/D) and who wrote its command.

use ds_core::time::stamp::Stamp;
use ds_core::word::Word;
use serde::{Deserialize, Serialize};
use temor_core::vocab::ids::{BlockId, ExitCode, GridSpan, TurnRef};
use temor_core::vocab::text::{AgentLabel, CommandText};

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

/// How a block stands, as the person sees it (the gutter mark's meaning).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Outcome {
    /// The command has not finished.
    Running,
    /// It exited with status 0.
    Succeeded,
    /// It exited with another status.
    Failed(ExitCode),
    /// It was stopped (Ctrl-C or a closed pipe), which is not a failure.
    Stopped(ExitCode),
    /// It ended, and the shell did not say how.
    Unreported,
}

impl Block {
    /// How the block stands.
    pub fn outcome(&self) -> Outcome {
        match (self.ended, self.exit) {
            (None, _) => Outcome::Running,
            (Some(_), Some(code)) if code.succeeded() => Outcome::Succeeded,
            (Some(_), Some(code)) if code.stopped() => Outcome::Stopped(code),
            (Some(_), Some(code)) => Outcome::Failed(code),
            (Some(_), None) => Outcome::Unreported,
        }
    }

    /// Milliseconds from start to end; `None` while it runs.
    pub fn elapsed_ms(&self) -> Option<u64> {
        self.ended.map(|end| end.since(self.started))
    }
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
