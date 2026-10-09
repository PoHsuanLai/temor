//! The crate's one error: what can be wrong with outside data as it becomes a core value.

use thiserror::Error;

/// Why outside data was refused at the boundary. Each variant names the fix a caller can make.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
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
    /// The chord belongs to the desktop (Super, or Ctrl+Space); a keymap may not bind it.
    #[error("{0} belongs to the desktop")]
    ReservedChord(String),
    /// The chord already runs another action.
    #[error("{chord} is already used by {by}")]
    ChordInUse { chord: String, by: String },
    /// A settings line names no setting this version knows.
    #[error("no setting called {0:?}")]
    UnknownSetting(String),
    /// A settings line has a value its setting cannot take.
    #[error("{key} cannot be {value:?}")]
    BadSettingValue { key: String, value: String },
    /// A key chord in a settings file did not parse ("ctrl+shift+c" is the form).
    #[error("not a key combination: {0:?}")]
    BadChord(String),
}
