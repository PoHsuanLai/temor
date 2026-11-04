//! What the window gives the desktop services, as plain data and one trait. The window
//! implements [`WindowPort`]; this crate turns agent requests into calls on it and its answers
//! into the agent layer's values. Nothing here knows the window's own types.

use std::future::Future;
use std::path::PathBuf;

/// A pane, by the number the window gives it. Agents hold these between calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PaneRef(pub u64);

/// A command (a block) in a pane, by the number the window gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CommandRef(pub u64);

/// What a pane holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PaneKind {
    /// A shell.
    Terminal,
    /// A coding agent.
    Agent,
    /// A file viewer.
    Viewer,
}

impl PaneKind {
    /// The word for this kind in the desktop's intents (`dist/intents/org.quire.Temor.toml`) and
    /// in the pane list an agent reads.
    pub fn word(self) -> &'static str {
        match self {
            Self::Terminal => "terminal",
            Self::Agent => "agent",
            Self::Viewer => "viewer",
        }
    }

    /// The kind a word names, if it names one.
    pub fn from_word(word: &str) -> Option<Self> {
        [Self::Terminal, Self::Agent, Self::Viewer]
            .into_iter()
            .find(|kind| kind.word() == word)
    }
}

/// Whether a pane has the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Focus {
    /// It has.
    Focused,
    /// It has not.
    Background,
}

/// Where a new pane goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Placement {
    /// A new tab.
    Tab,
    /// Beside the current pane.
    SplitRight,
    /// Below the current pane.
    SplitDown,
}

/// How a command ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Exit {
    /// Still running.
    Running,
    /// Finished with this status.
    Code(i32),
    /// Finished, and the shell did not say how.
    Unknown,
}

/// A pane as the window describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneInfo {
    /// Its number.
    pub id: PaneRef,
    /// What it is called.
    pub title: String,
    /// Its working folder, when the shell reported one.
    pub cwd: Option<PathBuf>,
    /// What it holds.
    pub kind: PaneKind,
    /// Whether it has the keyboard.
    pub focus: Focus,
}

/// A command as the window describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandInfo {
    /// Its number.
    pub id: CommandRef,
    /// The line that was run.
    pub line: String,
    /// How it ended.
    pub exit: Exit,
}

/// An app Space and the desktop Space it is linked to, if any. The agent layer's Space id is made
/// from these by porter; temor never formats it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SpaceRef {
    /// The app's own number for the Space.
    pub local: u64,
    /// The desktop-wide Space it is linked to.
    pub link: Option<String>,
}

/// What the person is looking at: the answer to `context()` (PLAN §8.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowContext {
    /// The window's title.
    pub title: String,
    /// The pane with the keyboard.
    pub focused: Option<PaneInfo>,
    /// The last command in that pane.
    pub last_command: Option<CommandInfo>,
    /// The selected text, if any.
    pub selection: Option<String>,
    /// How many panes are open.
    pub pane_count: u32,
    /// The Space the window is in.
    pub space: SpaceRef,
}

/// A run of lines in a pane's text, counted from the top of its history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LineRange {
    /// The first line.
    pub from: u32,
    /// How many lines.
    pub count: u32,
}

/// A request for a new pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneOpen {
    /// What to open.
    pub kind: PaneKind,
    /// Where to put it.
    pub placement: Placement,
    /// The folder to start in; the current pane's when absent.
    pub cwd: Option<PathBuf>,
}

/// A command to run in a sandbox (the overlay is the undo).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxRun {
    /// The line to run.
    pub line: String,
    /// The folder to run in; the focused pane's when absent.
    pub cwd: Option<PathBuf>,
    /// Whether it can reach the network.
    pub network_open: bool,
}

/// What a sandboxed run produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxResult {
    /// How it ended.
    pub exit: Exit,
    /// What it printed.
    pub output: String,
    /// What [`WindowPort::undo`] needs to take the run back, when it changed anything.
    pub undo: Option<String>,
}

/// Why the window did not do what was asked.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PortRefusal {
    /// No such pane.
    #[error("there is no such pane")]
    NoSuchPane,
    /// No such command.
    #[error("there is no such command")]
    NoSuchCommand,
    /// The window is busy or closing.
    #[error("temor is busy")]
    Busy,
    /// It cannot be done here, in words for the person.
    #[error("{0}")]
    Failed(String),
}

/// What the window offers the desktop services. Methods are called from the bus task, not the
/// window's thread, so an implementation keeps its answers in shared state (a snapshot behind a
/// lock) and returns quickly. What only the window's thread can answer (a pane's text, opening,
/// closing and typing into a pane) is a future: the bus task awaits it and never blocks its
/// worker.
pub trait WindowPort: Send + Sync + 'static {
    /// What the person is looking at now.
    fn context(&self) -> WindowContext;
    /// Every pane, in tab order.
    fn panes(&self) -> Vec<PaneInfo>;
    /// A run of a pane's text.
    fn read_pane(
        &self,
        pane: PaneRef,
        range: LineRange,
    ) -> impl Future<Output = Result<String, PortRefusal>> + Send;
    /// A pane's commands, oldest first.
    fn commands(&self, pane: PaneRef) -> Result<Vec<CommandInfo>, PortRefusal>;
    /// One command's output.
    fn read_command(
        &self,
        pane: PaneRef,
        command: CommandRef,
    ) -> impl Future<Output = Result<String, PortRefusal>> + Send;
    /// Open a pane; the answer is the new pane.
    fn open_pane(
        &self,
        open: PaneOpen,
    ) -> impl Future<Output = Result<PaneRef, PortRefusal>> + Send;
    /// Close a pane an agent opened (its undo).
    fn close_pane(&self, pane: PaneRef) -> impl Future<Output = Result<(), PortRefusal>> + Send;
    /// Type `line` and Enter into a pane, as the person would.
    fn run_in_pane(
        &self,
        pane: PaneRef,
        line: &str,
    ) -> impl Future<Output = Result<(), PortRefusal>> + Send;
    /// Run a command in a sandbox.
    fn run_sandboxed(
        &self,
        run: SandboxRun,
    ) -> impl Future<Output = Result<SandboxResult, PortRefusal>> + Send;
    /// Take a sandboxed run back with the token it gave.
    fn undo(&self, token: &str) -> Result<(), PortRefusal>;
}
