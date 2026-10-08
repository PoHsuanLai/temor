//! The crate's one error type.

use std::io;
use std::path::PathBuf;

/// What went wrong, shaped so the caller can act on it.
#[derive(Debug, thiserror::Error)]
pub enum PtyError {
    /// This platform has no pseudo-terminal support in temor yet (macOS, Windows and every OS
    /// other than Linux).
    #[error("terminals are not supported on this platform yet")]
    Unsupported,
    /// The pseudo-terminal pair could not be opened or set up.
    #[error("could not open a terminal: {0}")]
    Open(#[source] io::Error),
    /// The launcher or program could not be started.
    #[error("could not start {program}: {source}")]
    Spawn {
        program: PathBuf,
        #[source]
        source: io::Error,
    },
    /// Reading, writing or waiting on the terminal failed for a reason other than its closing.
    #[error("terminal input or output failed: {0}")]
    Io(#[source] io::Error),
    /// The window size could not be set.
    #[error("could not resize the terminal: {0}")]
    Resize(#[source] io::Error),
    /// A signal could not be delivered to the child's process group.
    #[error("could not signal the program: {0}")]
    Signal(#[source] io::Error),
    /// The child has been reaped, so its process group may no longer be ours to signal.
    #[error("the program has already exited")]
    Reaped,
    /// The foreground process group could not be read.
    #[error("could not read the foreground process: {0}")]
    Foreground(#[source] io::Error),
    /// The shell integration files could not be written into the runtime directory.
    #[error("could not write shell integration files to {dir}: {source}")]
    Shims {
        dir: PathBuf,
        #[source]
        source: io::Error,
    },
    /// The session-leader trampoline was called without a program to run.
    #[error("the session launcher was given no program")]
    NoProgram,
    /// The session-leader trampoline could not make a new session on its terminal.
    #[error("could not take the terminal as a new session: {0}")]
    SessionLeader(#[source] io::Error),
}
