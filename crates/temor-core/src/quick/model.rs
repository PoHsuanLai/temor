//! The quick terminal's state, inputs and outputs.

use crate::vocab::ids::PaneId;
use crate::vocab::text::Cwd;
use ds_style::space::list::SpaceId;

/// The quick terminal.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum QuickState {
    /// No shell running.
    #[default]
    Closed,
    /// Shown over the window, with its shell running.
    Open(QuickTerm),
    /// Not shown, with its shell still running: the hotkey brings it back as it was. It is a
    /// throwaway: never saved with the session, only kept while temor runs.
    Hidden(QuickTerm),
}

/// An open quick terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickTerm {
    /// Its pane. The window took the id from the same counter as every pane, so promoting the
    /// pane into a tab needs no new id.
    pub pane: PaneId,
    /// The folder its shell is in, as far as it is known (it follows the shell's reports).
    pub cwd: Option<Cwd>,
}

/// What moves the quick terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuickIn {
    /// The hotkey. Closed: open a shell in `cwd` (the folder of the pane in front) as `pane`, an
    /// id the caller allocated. Open: hide it, the shell kept running. Hidden: show the same
    /// shell again. The hotkey is the way out; Escape and Ctrl-C belong to the shell.
    Toggle {
        /// The pane to use when it opens.
        pane: PaneId,
        /// Where its shell starts.
        cwd: Option<Cwd>,
    },
    /// The shell in this pane ended by itself.
    ShellExited(PaneId),
    /// The shell in this pane reported a new folder.
    Cwd {
        /// The pane.
        pane: PaneId,
        /// Its folder now.
        cwd: Cwd,
    },
    /// "Move to Space": keep this terminal, running, as a tab in that Space.
    MoveToSpace(SpaceId),
}

/// What the quick terminal asks the window to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuickOut {
    /// Show the overlay and start a shell in this pane.
    Spawn {
        /// The pane.
        pane: PaneId,
        /// The folder to start in.
        cwd: Option<Cwd>,
    },
    /// Drop this pane: its shell ended or was handed on, so nothing of it is kept.
    End(PaneId),
    /// Show the overlay again over the shell that is still running.
    Show(PaneId),
    /// Hide the overlay; the shell is not touched.
    Hidden,
    /// Give the running pane to `space` as a tab of its own (see [`adopt`](super::adopt)),
    /// without restarting its shell.
    Promote {
        /// The pane, still running.
        pane: PaneId,
        /// The Space that takes it.
        space: SpaceId,
        /// Its folder, for the new tab.
        cwd: Option<Cwd>,
    },
}
