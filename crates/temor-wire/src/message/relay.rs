//! The agent relay: the agent's file and terminal calls, forwarded to a remote workspace.
//!
//! The names follow the agent client protocol's `fs/*` and `terminal/*` calls, so the local
//! executor can pass them through without translation.

use super::files::RemotePath;
use super::session::{EnvVar, ExitStatus};
use crate::ids::TerminalId;
use crate::units::{ByteCount, LineCount, LineNumber};
use serde::{Deserialize, Serialize};

/// One call from the agent, carried out on the host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelayCall {
    /// `fs/read_text_file`: a file's text, optionally from a line for a number of lines.
    ReadText {
        /// The file.
        path: RemotePath,
        /// The first line to return, or `None` for the start.
        line: Option<LineNumber>,
        /// How many lines, or `None` for all.
        limit: Option<LineCount>,
    },
    /// `fs/write_text_file`: replace a file's text.
    WriteText {
        /// The file.
        path: RemotePath,
        /// The whole new text.
        content: String,
    },
    /// `terminal/create`: start a command in a remote terminal.
    CreateTerminal {
        /// The program.
        command: String,
        /// Its arguments.
        args: Vec<String>,
        /// Its environment.
        env: Vec<EnvVar>,
        /// Its folder, or `None` for the workspace root.
        cwd: Option<RemotePath>,
        /// The most output kept; beyond it the head is cut.
        output_limit: ByteCount,
    },
    /// `terminal/output`: what the terminal has printed so far.
    TerminalOutput {
        /// The terminal.
        terminal: TerminalId,
    },
    /// `terminal/wait_for_exit`: block until the command ends.
    WaitForExit {
        /// The terminal.
        terminal: TerminalId,
    },
    /// `terminal/kill`: stop the command, keeping the terminal.
    KillTerminal {
        /// The terminal.
        terminal: TerminalId,
    },
    /// `terminal/release`: forget the terminal.
    ReleaseTerminal {
        /// The terminal.
        terminal: TerminalId,
    },
}

/// Whether a terminal's kept output was cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Truncation {
    /// Everything printed is kept.
    Kept,
    /// The head was cut to fit the limit.
    Cut,
}

/// The answer to a relayed call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelayReply {
    /// The text of a file.
    Text(String),
    /// A write landed.
    Written,
    /// A terminal was started.
    Terminal(TerminalId),
    /// A terminal's output so far.
    Output {
        /// The text.
        text: String,
        /// Whether it was cut.
        truncation: Truncation,
        /// The exit status, if the command has ended.
        exit: Option<ExitStatus>,
    },
    /// The command's exit status.
    Exit(ExitStatus),
    /// The call was carried out and has nothing to return.
    Done,
}
