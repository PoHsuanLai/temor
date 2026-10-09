//! Errors: a request that failed, with the typed reason. A response carries one of these in place
//! of its reply, under the same request id.

use super::files::{FileStamp, RemotePath};
use super::trial::UncheckedWhy;
use crate::hello::Capability;
use crate::ids::{SessionId, TerminalId, WatchId};
use serde::{Deserialize, Serialize};

/// Why the host refused a relayed call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Refusal {
    /// The path is outside the remote workspace.
    OutsideWorkspace,
    /// The host's policy does not allow this call.
    DeniedByHost,
    /// The call's output or size is over the host's limit.
    OverLimit,
    /// A reason this build does not know.
    #[serde(other)]
    Unknown,
}

/// Why a request failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum RemoteError {
    /// The helper cannot read this request: it is from a newer temor. Sent under the request's id.
    UnknownRequest,
    /// No session has that id (it ended, or it was never opened here).
    NoSuchSession {
        /// The id asked for.
        session: SessionId,
    },
    /// No watch has that id.
    NoSuchWatch {
        /// The id asked for.
        watch: WatchId,
    },
    /// No relayed terminal has that id.
    NoSuchTerminal {
        /// The id asked for.
        terminal: TerminalId,
    },
    /// The path does not exist.
    NotFound {
        /// The path.
        path: RemotePath,
    },
    /// The host's permissions refuse the path.
    PermissionDenied {
        /// The path.
        path: RemotePath,
    },
    /// A write's check failed: the file is not what the writer last saw. `current` is what is
    /// there now, or `None` if it was removed.
    Conflict {
        /// The file's stamp now.
        current: Option<FileStamp>,
    },
    /// The host refused a relayed call, and says why.
    HostRefused {
        /// The reason.
        reason: Refusal,
    },
    /// The helper does not offer this capability (it was not agreed in the handshake).
    Unsupported {
        /// The capability asked for.
        capability: Capability,
    },
    /// A trial could not be run here; the reason is the same one a verdict would carry.
    TrialUnavailable {
        /// Why.
        reason: UncheckedWhy,
    },
    /// The request names a kind this helper knows, but a field inside it is one this helper cannot
    /// read (it comes from a newer temor). The request was not run.
    Unreadable {
        /// What was not understood, for a log.
        detail: String,
    },
    /// The request was well formed JSON but not something the helper can do.
    BadRequest {
        /// What was wrong, for a log.
        detail: String,
    },
    /// An operating-system error the variants above do not name.
    Io {
        /// The error, for a log.
        detail: String,
    },
}
