//! Sessions: the shells and programs temor runs on the host.

use super::files::RemotePath;
use super::palette::Palette;
use crate::ids::SessionId;
use crate::units::{CellPx, ExitCode, Seconds, Size};
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
    /// One cell in pixels, for the program's pixel size reports and its terminal's window size.
    /// A client that sends none gets 8 by 16.
    #[serde(default)]
    pub cell: CellPx,
    /// The window's colours, so a program that asks for them at start already gets the right
    /// answer. `None` keeps the terminal's defaults until [`Request::SetPalette`](super::Request).
    #[serde(default)]
    pub palette: Option<Palette>,
}

/// Whether a session has a client on it right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SessionState {
    /// A client is attached and receives output.
    Attached,
    /// No client is attached; the session runs on and keeps its screen.
    Detached,
    /// A state this build does not know.
    #[serde(other)]
    Unknown,
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
