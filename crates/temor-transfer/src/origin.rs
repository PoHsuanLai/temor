//! Where a download came from: the shelf shows it and jumps back to it.

use serde::{Deserialize, Serialize};

/// A host the transfer came from, as the person named it (an ssh alias or a host name).
pub use temor_schema::text::HostName;

/// The tab a local pane belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TabKey(u64);

impl TabKey {
    pub const fn new(key: u64) -> Self {
        Self(key)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A pane within a tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PaneKey(u64);

impl PaneKey {
    pub const fn new(key: u64) -> Self {
        Self(key)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A command block (OSC 133) whose output was saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BlockKey(u64);

impl BlockKey {
    pub const fn new(key: u64) -> Self {
        Self(key)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Where a shelf item came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Origin {
    /// A file a program in a local pane sent (`temor get`, `temor save`, a pipe).
    LocalPane { tab: TabKey, pane: PaneKey },
    /// A file a remote host sent over the terminal stream.
    Host { host: HostName },
    /// "Save output" on a command block.
    Save { block: BlockKey },
}
