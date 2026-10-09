//! Trial runs: a proposed command run on the host in its sandbox, and the verdict.
//!
//! The verdict mirrors temor-core's `TrialVerdict` (PLAN §3) in name and shape, so a remote
//! verdict and a local one read the same. A run that cannot be judged says why, and never counts
//! as a pass.

use super::files::RemotePath;
use crate::units::ExitCode;
use ds_core::word::Word;
use serde::{Deserialize, Serialize};

/// A command to trial, and the folder it would run in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrialRequest {
    /// The command line, as the person would type it.
    pub command: String,
    /// The folder the trial runs in.
    pub cwd: RemotePath,
}

/// A path relative to the trial's overlaid folder, as UTF-8.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RelPath(pub String);

/// The first error line a failed trial printed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ErrorLine(pub String);

/// How a trial changed a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word, Serialize, Deserialize)]
pub enum ChangeKind {
    /// Made it.
    Created,
    /// Changed it.
    Modified,
    /// Deleted it.
    Removed,
    /// A kind this build does not know.
    #[serde(other)]
    Unknown,
}

/// One file a trial changed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileChange {
    /// Where, relative to the overlaid folder.
    pub path: RelPath,
    /// How.
    pub how: ChangeKind,
}

/// The files a clean trial changed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct FileChanges {
    /// Each changed file, in the order the trial reported them.
    #[serde(default)]
    pub files: Vec<FileChange>,
}

/// The outcome of a trial.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrialVerdict {
    /// It ran and exited zero, and these files changed.
    Clean {
        /// What it changed.
        changes: FileChanges,
    },
    /// It ran and exited non-zero.
    Failed {
        /// The exit code.
        exit: ExitCode,
        /// The first error it printed.
        first_error: ErrorLine,
    },
    /// It could not be judged.
    Unchecked(UncheckedWhy),
}

/// Why a trial could not be judged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word, Serialize, Deserialize)]
pub enum UncheckedWhy {
    /// It ran past the time limit.
    TimedOut,
    /// It failed in a way that needed the network, which a trial does not have.
    NeededNetwork,
    /// The host has no usable sandbox (no bwrap, or unprivileged user namespaces are off).
    NoSandbox,
    /// A reason this build does not know.
    #[serde(other)]
    Unknown,
}
