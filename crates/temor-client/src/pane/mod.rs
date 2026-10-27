//! A pane as the client keeps it (PLAN §2b, P24c): what the pane is, apart from how a window
//! shows it. Its engine, what its program said (title, folder, progress, how it ended), its
//! commands, its shell's state, its completion sources, what was answered about its clipboard,
//! and when a finished command is worth a desktop notice. The window holds one beside its own
//! state for the pane (the painter, the selection, find, the composer).

pub mod blocks;
pub mod clipboard;
pub mod clock;
mod hear;
mod heard;
pub mod marks;
pub mod notify;
mod open;
pub mod sources;

use std::path::PathBuf;

use temor_core::notify::{Focus, Limiter};
use temor_pane::settings::NotifyAfter;
use temor_pane::{Hold, PaneHandle};
use temor_pty::ChildExit;
use temor_vt::Progress;

use blocks::BlockLog;
use clipboard::PendingClipboard;
use clock::Clock;
use notify::Notifier;
use sources::Sources;

pub use hear::{Heard, desktop_note};
pub use heard::{SNAPSHOT_LIMIT, sanitise_title};
pub use open::{OpenError, PaneKit};

/// What a pane is, apart from how the window shows it: its engine, what its program said (title,
/// folder, progress, how it ended), its commands, its shell's state, its completion sources, what
/// was answered about its clipboard, and when a finished command is worth a desktop notice.
pub struct PaneCore {
    pub pane: PaneHandle,
    pub title: String,
    pub lifetime: Lifetime,
    pub hold: Hold,
    pub hold_generation: u64,
    pub blocks: BlockLog,
    /// What the shell holds that a new shell would need (folder, variables, aliases).
    pub shell_state: temor_core::session::ShellState,
    /// The `PATH` the shell last reported: where its programs are found, which a window opened
    /// from the Finder is not given (its own is the system's few folders).
    pub shell_path: Option<String>,
    /// Commands the shell this session continues ran before this window began.
    pub inherited: Vec<temor_core::session::CommandRecord>,
    /// The key temor gave the shell it started here; a state report without it is not the
    /// shell's and is dropped. None: nothing reports.
    pub report_key: Option<temor_shell::report::ReportKey>,
    /// The sources that complete the composer's line besides the shell's own.
    pub sources: Sources,
    /// The pane's folder as the shell last said it, to run a completion in.
    pub folder: Option<std::path::PathBuf>,
    pub cwd: Option<String>,
    pub progress: Option<Progress>,
    pub notify_after: NotifyAfter,
    pub limiter: Limiter,
    pub notifier: Box<dyn Notifier>,
    pub clock: Box<dyn Clock>,
    pub shelf_dir: PathBuf,
    /// How the pane's shell quotes a word, for pasting dropped paths.
    pub quoting: temor_core::shelf::Quoting,
    /// Whether the pane's shell runs on this computer.
    pub site: temor_core::shelf::PaneSite,
    /// What the person answered about the program copying to the clipboard (OSC 52).
    pub clip_write: temor_core::remote::Grant,
    /// The pane's copy of the clipboard, for the program's reads.
    pub clip_mirror: temor_pane::ClipboardMirror,
    /// A clipboard question waiting for the person.
    pub clip_pending: Option<PendingClipboard>,
    /// Whether the pane is the one the person is looking at (a pane that is not notifies).
    pub focus: Focus,
    /// What the person last sent as a command, until the command it starts takes it: a shell
    /// that does not say what it runs (nushell) shows a command of several lines on rows the
    /// screen cannot give back.
    pub sent: Option<String>,
    /// A message for the person from the pane (a notice the desktop refused), until the window
    /// takes it.
    pub said: Option<String>,
}

/// Whether the program is still running, and how it ended if not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifetime {
    Running,
    Ended(Option<ChildExit>),
}

impl std::fmt::Debug for PaneCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaneCore")
            .field("title", &self.title)
            .field("lifetime", &self.lifetime)
            .field("cwd", &self.cwd)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    /// The core can be held by a thread of the client's (P24c: the panes' events are heard there).
    #[test]
    fn a_pane_core_can_move_to_another_thread() {
        fn sendable<T: Send>() {}
        sendable::<super::PaneCore>();
    }
}
