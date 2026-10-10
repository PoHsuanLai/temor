//! What a pane tells the window. Callbacks never block: the I/O thread sends these and moves on.

use super::xfer::TransferEvent;
use temor_pty::ChildExit;
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
    /// The cursor, where mark `seq` stood, anchored.
    Anchor { seq: u64, anchor: AnchorId },
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
    /// The name of the pane's foreground process changed (`None`: it could not be read). Sent by
    /// the I/O thread about once a second, only on a change.
    Foreground(Option<String>),
    /// A report over the pane's status socket (an adapter, a hook or the `temor` command).
    #[cfg(unix)]
    Reported(crate::report::StatusLine),
}

/// Whether the program is holding the screen for a synchronized update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Hold {
    /// Keep showing the last frame until the program releases it (or the window's timeout).
    Held,
    /// Draw again.
    Released,
}
