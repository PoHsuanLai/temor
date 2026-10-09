//! Trial runs: a proposed command run on the host in its sandbox, and the verdict.
//!
//! The verdict mirrors the local trial (PLAN §3): a run that cannot be judged says why, and never
//! counts as a pass.

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

/// The outcome of a trial.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Verdict {
    /// It ran and exited zero.
    Passed,
    /// It ran and exited non-zero.
    Failed {
        /// The exit code.
        exit: ExitCode,
    },
    /// It could not be judged.
    Unchecked {
        /// Why.
        reason: UncheckedWhy,
    },
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
}
