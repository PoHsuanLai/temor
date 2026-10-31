//! The plain values the pseudo-terminal takes and reports, on every platform.

use std::ffi::OsString;
use std::path::PathBuf;

use crate::size::GridSize;
use temor_schema::process::CommandLine;
use temor_shell::ChildEnv;

/// What to start on a new terminal: an explicit program, arguments, directory and the complete
/// environment (nothing is inherited from temor).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnSpec {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
    pub env: ChildEnv,
    pub size: GridSize,
}

/// A process id (also a process group id: the child leads its own group and session).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProcessId(pub i32);

/// How the child ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChildExit {
    /// It exited with this status.
    Exited(i32),
    /// A signal with this number ended it.
    Signaled(i32),
}

/// The signals temor sends to a terminal's process group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignalKind {
    /// The terminal went away (closing a tab); shells pass it on to their jobs.
    Hangup,
    /// Ctrl-C, sent from outside the line discipline.
    Interrupt,
    /// A polite request to stop.
    Terminate,
    /// Stop now; cannot be caught.
    Kill,
}

/// Where the process filesystem is mounted (normally `/proc`), injected so tests can fake it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcRoot(pub PathBuf);

/// The terminal's foreground process group, for tab titles and the running row. The name and
/// directory are those of the group leader, read from `/proc` on Linux; elsewhere, or when the
/// process has gone, they are `None`. The command line is that of the process whose name is
/// given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Foreground {
    pub group: ProcessId,
    pub name: Option<String>,
    pub cwd: Option<PathBuf>,
    pub command: Option<CommandLine>,
}

/// What to wait for in [`PtyIo::wait`](crate::PtyIo::wait).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Interest {
    Read,
    Write,
    ReadWrite,
}

/// Whether one direction can make progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ready {
    /// A read or write now will not block (a read may report [`ReadOutcome::Closed`]).
    Now,
    /// Not yet.
    Later,
}

/// What [`PtyIo::wait`](crate::PtyIo::wait) found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Readiness {
    pub read: Ready,
    pub write: Ready,
}

/// The result of one nonblocking read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReadOutcome {
    /// This many bytes were read into the buffer.
    Data(usize),
    /// Nothing to read yet.
    WouldBlock,
    /// Every process holding the terminal has closed it; no more output will come.
    Closed,
}

/// The result of one nonblocking write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WriteOutcome {
    /// This many bytes were written; the caller keeps the rest for later.
    Wrote(usize),
    /// The terminal's input queue is full.
    WouldBlock,
    /// The terminal has gone.
    Closed,
}
