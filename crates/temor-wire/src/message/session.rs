//! Sessions: the shells and programs temor runs on the host.

use super::files::RemotePath;
use crate::ids::SessionId;
use crate::units::{ExitCode, Seconds, Size};
use ds_core::word::Word;
use serde::{Deserialize, Serialize};

/// One environment variable for a session or a relayed command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvVar {
    /// The variable's name.
    pub name: String,
    /// Its value.
    pub value: String,
}

/// How long a detached session is kept before the helper ends it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IdleLimit {
    /// Keep detached sessions until they exit.
    Never,
    /// End a session that has been detached for this long.
    After(Seconds),
}

/// What temor asks for when it opens a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenSession {
    /// The program to run, or `None` for the account's login shell.
    pub shell: Option<RemotePath>,
    /// The folder to start in.
    pub cwd: RemotePath,
    /// The environment, complete: the helper adds nothing of its own.
    pub env: Vec<EnvVar>,
    /// The starting size.
    pub size: Size,
}

/// Whether a session has a client on it right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word, Serialize, Deserialize)]
pub enum SessionState {
    /// A client is attached and receives output.
    Attached,
    /// No client is attached; the session runs on and keeps its screen.
    Detached,
}

/// One session in a listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInfo {
    /// The session.
    pub session: SessionId,
    /// Its folder when the listing was made.
    pub cwd: RemotePath,
    /// Its size.
    pub size: Size,
    /// Whether a client is attached.
    pub state: SessionState,
}

/// How a program ended.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExitStatus {
    /// It exited with a code.
    Code(ExitCode),
    /// It was ended by a signal, named as the host names it (for example `SIGKILL`).
    Signal {
        /// The signal's name.
        name: String,
    },
}
