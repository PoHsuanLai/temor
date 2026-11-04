//! The crate's one error: what can be wrong with outside data as it becomes a core value.

use thiserror::Error;

/// Why outside data was refused at the boundary. Each variant names the fix a caller can make.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum CoreError {
    /// A schema value refused its text (an environment key with `=`, a URL that is not one).
    #[error(transparent)]
    Schema(#[from] temor_schema::error::SchemaError),
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
}
