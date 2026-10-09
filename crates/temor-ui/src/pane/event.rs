//! What a pane tells the window. Callbacks never block: the I/O thread sends these and moves on.

use temor_pty::ChildExit;
use temor_vt::{AnchorId, ClipboardWrite, Notification, Progress, PromptEvent};

/// One event from a pane's terminal or its program.
#[derive(Debug)]
pub enum PaneEvent {
    /// The screen changed: repaint it.
    Dirty,
    /// The program set the window title (OSC 0/2).
    Title(String),
    /// The bell rang.
    Bell,
    /// The working directory changed (OSC 7), as the raw URL.
    Cwd(String),
    /// The program asked to write the clipboard (OSC 52). Not acted on yet (PLAN §6).
    Clipboard(ClipboardWrite),
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
}

/// Whether the program is holding the screen for a synchronized update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Hold {
    /// Keep showing the last frame until the program releases it (or the window's timeout).
    Held,
    /// Draw again.
    Released,
}
