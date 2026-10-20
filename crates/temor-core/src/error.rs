//! The crate's one error: what can be wrong with outside data as it becomes a core value.

use thiserror::Error;

/// Why outside data was refused at the boundary. Each variant names the fix a caller can make.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum CoreError {
    /// An environment key was empty or held `=` or NUL; the profile entry is dropped.
    #[error("not an environment variable name: {0:?}")]
    BadEnvKey(String),
    /// A stored split tree did not describe one tree over distinct panes; the tab opens with a
    /// fresh single pane.
    #[error("the stored split layout is damaged: {0}")]
    BadTree(&'static str),
    /// The pane is not in this tab's tree; the caller's view of the tab is stale.
    #[error("pane {0} is not in this tab")]
    UnknownPane(u64),
    /// The new pane's id is already in the tree; the caller must allocate a fresh one.
    #[error("pane {0} is already in this tab")]
    DuplicatePane(u64),
    /// A tab or pane counter has no id left to hand out; nothing was changed.
    #[error("no ids are left to hand out")]
    IdsExhausted,
    /// The tab is not in this workspace; the caller's view of the sidebar is stale.
    #[error("tab {0} is not in this Space")]
    UnknownTab(u64),
    /// The Space is not in the list (it was deleted); the caller's view of the Spaces is stale.
    #[error("Space {0} is not in the list")]
    UnknownSpace(u64),
    /// The tab already holds the most panes it can; open a new tab instead.
    #[error("this tab cannot be split further")]
    TreeFull,
    /// The tab's only pane cannot be torn off; closing the tab is the way to remove it.
    #[error("a tab's only pane cannot be torn off")]
    LastPane,
    /// An account reference was not an account id (lowercase letters, digits, `.`, `_`, `-`,
    /// at most 64 bytes, starting with a letter or digit); the credential entry is dropped.
    #[error("not an account id: {0:?}")]
    BadAccountRef(String),
    /// A Space's SSH agent socket would not fit in a socket address; the agent is not started
    /// and the Space opens without one.
    #[error("the SSH agent socket path is {len} bytes, past the {max} a socket allows")]
    SocketPathTooLong {
        /// The path's length in bytes.
        len: usize,
        /// The longest a socket path may be.
        max: usize,
    },
    /// The dotenv file named by a profile is not under the project root (absolute, or with `..`);
    /// it is not read.
    #[error("the dotenv path leaves the project folder: {0:?}")]
    BadDotenvPath(String),
    /// The session could not be written as text (a value the format cannot hold).
    #[error("the session could not be written: {0}")]
    SessionEncode(String),
    /// git could not be started (the binary path is wrong or not executable).
    #[error("git could not be run: {0}")]
    GitNotRun(String),
    /// git ran and refused. `what` is the command's first words; `stderr` its message, cut short.
    #[error("git {what} failed: {stderr}")]
    GitFailed { what: String, stderr: String },
    /// git printed something this code does not know how to read.
    #[error("unexpected output from git {what}")]
    GitOutput { what: String },
    /// A ref or object id did not name a commit in the repository.
    #[error("not a commit: {0}")]
    NotACommit(String),
    /// A worktree name was empty after cleaning, or a path would leave temor's folder.
    #[error("not a usable worktree name: {0:?}")]
    BadWorktreeName(String),
}

/// How much of git's message is kept in an error.
const STDERR_KEEP: usize = 300;
/// How much of a rejected ref or id is kept in an error.
const REV_KEEP: usize = 80;

impl CoreError {
    /// git ran `what` and refused with `stderr`, cut short.
    pub fn git_failed(what: impl Into<String>, stderr: &str) -> Self {
        Self::GitFailed {
            what: what.into(),
            stderr: stderr.chars().take(STDERR_KEEP).collect(),
        }
    }

    /// `text` did not name a commit; the first of it is kept.
    pub fn not_a_commit(text: &str) -> Self {
        Self::NotACommit(text.chars().take(REV_KEEP).collect())
    }
}
