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
    /// The tab already holds the most panes it can; open a new tab instead.
    #[error("this tab cannot be split further")]
    TreeFull,
    /// The tab's only pane cannot be torn off; closing the tab is the way to remove it.
    #[error("a tab's only pane cannot be torn off")]
    LastPane,
    /// The session could not be written as text (a value the format cannot hold).
    #[error("the session could not be written: {0}")]
    SessionEncode(String),
}
