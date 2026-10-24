//! What a pane tells the window. Callbacks never block: the I/O thread sends these and moves on.

use std::time::Instant;

use super::xfer::TransferEvent;
use temor_pty::{ChildExit, CommandLine};
use temor_vt::{
    AnchorId, ClipboardRead, ClipboardWrite, Notification, ProgramStatus, Progress, PromptEvent,
};

/// One event from a pane's terminal or its program.
#[derive(Debug)]
#[non_exhaustive]
pub enum PaneEvent {
    /// The screen changed: repaint it.
    Dirty,
    /// The program set the window title (OSC 0/2).
    Title(String),
    /// The bell rang.
    Bell,
    /// The working directory changed (OSC 7), as the raw URL.
    Cwd(String),
    /// The shell reported its variables and aliases (OSC 7503): the report's text, to be read
    /// with `temor_shell::report::decode`.
    ShellReport(String),
    /// The program asked to write the clipboard (OSC 52); the window decides (PLAN §6).
    Clipboard(ClipboardWrite),
    /// The program asked to read the clipboard before the person was asked: the window asks,
    /// and sets the answer on the pane's [`crate::ClipboardMirror`].
    ClipboardRead(ClipboardRead),
    /// A desktop notification from the program (OSC 9, OSC 777).
    Notification(Notification),
    /// A shell integration mark (OSC 133) starts, numbered `seq`. Its own event follows.
    MarkBegin { seq: u64 },
    /// A shell integration mark (OSC 133). Its anchor follows as `Anchor` with the same number.
    Prompt(PromptEvent),
    /// The cursor, where mark `seq` stood, anchored; `at` is when the pane read the mark, so a
    /// command's time does not take in how long its marks waited for the window.
    Anchor {
        seq: u64,
        anchor: AnchorId,
        at: Instant,
    },
    /// A progress report (OSC 9;4).
    Progress(Progress),
    /// The terminal was reset (RIS): a running command is no longer on screen.
    Reset,
    /// The program ended and was reaped; `None` when its status could no longer be read.
    Exited(Option<ChildExit>),
    /// The program holds the screen (synchronized output) or released it.
    RenderHold(Hold),
    /// A program offers a file, sends one, or asks for one (PLAN §9).
    Transfer(TransferEvent),
    /// A program status report (OSC 7501): what an agent says it is doing.
    Program(ProgramStatus),
    /// The pane's foreground process changed, by name or by command line (`None`: it could not
    /// be read). Sent by the I/O thread about once a second, only on a change.
    Foreground(Option<SeenForeground>),
    /// A report over the pane's status socket (an adapter, a hook or the `temor` command).
    #[cfg(unix)]
    Reported(crate::report::StatusLine),
}

/// The pane's foreground process as seen: its name, and its command line when it could be read.
/// Under a sandbox the name is the wrapper's (`bwrap`), and the program shows in the arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeenForeground {
    pub name: String,
    pub command: Option<CommandLine>,
}

/// Whether the program is holding the screen for a synchronized update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Hold {
    /// Keep showing the last frame until the program releases it (or the window's timeout).
    Held,
    /// Draw again.
    Released,
}
