//! A command proposal (§8.3) and its trial verdict.

use super::sanitise::SanitisedLine;
use crate::block::Taint;
use crate::vocab::ids::{ExitCode, TurnRef};
use crate::vocab::text::{ErrorLine, RelPath};
use ds_core::word::Word;
use serde::{Deserialize, Serialize};

/// A command the person asked for, cleaned and trial-run, ready to be typed into the prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Proposal {
    /// The command, one line with no control characters.
    pub text: SanitisedLine,
    /// The model turn that wrote it.
    pub turn: TurnRef,
    /// Whether that turn read untrusted text.
    pub taint: Taint,
    /// What the trial run found.
    pub verdict: TrialVerdict,
}

/// What running a proposal in the sandbox found (D7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum TrialVerdict {
    /// It exited zero; these are the files it would change.
    Clean {
        /// The files it changed in the overlay.
        changes: FileChanges,
    },
    /// It exited non-zero.
    Failed {
        /// Its exit status.
        exit: ExitCode,
        /// The first error it printed.
        first_error: ErrorLine,
    },
    /// It could not be checked.
    Unchecked(UncheckedWhy),
}

/// Why a proposal could not be checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum UncheckedWhy {
    /// It ran past the time limit.
    #[word(label = "Not checked: took too long")]
    TimedOut,
    /// It failed after trying to reach the network, which the trial has none of.
    #[word(label = "Not checked: needs the network")]
    NeededNetwork,
    /// No sandbox here (not Linux, or user namespaces are blocked).
    #[word(label = "Not checked")]
    NoSandbox,
    /// It is for another computer, which has no checker.
    #[word(label = "Not checked (remote)")]
    Remote,
}

/// The files a clean trial changed.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FileChanges {
    /// Each changed file, in the order the trial reported them.
    #[serde(default)]
    pub files: Vec<FileChange>,
}

/// One file a trial changed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileChange {
    /// Where, relative to the overlaid folder.
    pub path: RelPath,
    /// How.
    pub how: ChangeKind,
}

/// How a trial changed a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum ChangeKind {
    /// Made it.
    Created,
    /// Changed it.
    Modified,
    /// Deleted it.
    Removed,
}
