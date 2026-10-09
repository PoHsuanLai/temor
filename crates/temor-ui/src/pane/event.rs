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
    /// A shell integration mark (OSC 133). The cursor's anchor for it follows as `Anchor`.
    Prompt(PromptEvent),
    /// The cursor was anchored right after the mark before it (see `temor_vt::AnchorId`).
    Anchor(AnchorId),
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
