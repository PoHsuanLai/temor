//! The shelf's data. Newest first; every item has a path under the shelf's folder.

use super::naming::candidate_name;
use crate::transfer::Origin;
use crate::vocab::ids::{Bytes, DownloadId};
use crate::vocab::text::ShelfPath;
use ds_core::time::stamp::Stamp;
use serde::{Deserialize, Serialize};

/// How an item reached the shelf.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// A program sent it with the kitty file transfer sequence.
    Kitty,
    /// A program sent it with the iTerm2 `File=` sequence.
    Osc1337,
    /// The person ran `temor save`.
    TemorSave,
}

/// Why an item did not arrive. Each reads as a plain sentence in the shelf.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailWhy {
    /// The person said no.
    Refused,
    /// The sender stopped or the connection dropped part way.
    Broken,
    /// The file is bigger than temor accepts.
    TooBig,
    /// The folder is full, missing or not writable.
    Unwritable,
}

/// How far an item got.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum ItemState {
    /// A name is chosen; the edge has not yet confirmed the file.
    Waiting,
    /// Arriving. `total` is what the sender said it would send, if it said.
    Receiving {
        received: Bytes,
        total: Option<Bytes>,
    },
    /// Saved and complete.
    Ready { size: Bytes },
    /// Did not arrive.
    Failed { why: FailWhy },
}

/// Whether the person has opened the item since it arrived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Seen {
    #[default]
    New,
    Opened,
}

/// An offer to put a file on the shelf, after any consent it needed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Incoming {
    pub source: Source,
    pub origin: Origin,
    /// The name the sender gave; the shelf makes it safe.
    pub name: String,
    pub size: Option<Bytes>,
    pub at: Stamp,
}

/// One item on the shelf.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShelfItem {
    pub id: DownloadId,
    /// The safe name the sender's name became, before any " (2)" is added.
    pub name: String,
    /// Where the edge was told to create it.
    pub path: ShelfPath,
    pub origin: Origin,
    pub source: Source,
    pub at: Stamp,
    pub state: ItemState,
    /// Which candidate name is in use: 0 is the name itself, 1 is " (2)", and so on.
    pub attempt: u32,
    #[serde(default)]
    pub seen: Seen,
}

/// What to remove from the shelf. Files on disk stay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Clear {
    One(DownloadId),
    /// Everything that is ready or failed; what is arriving stays.
    Finished,
    All,
}

/// The shelf: the folder it saves into (injected) and its items, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shelf {
    pub(super) dir: ShelfPath,
    pub(super) items: Vec<ShelfItem>,
    pub(super) next: u64,
}

impl Shelf {
    /// An empty shelf that saves into `dir` (the person's Downloads folder).
    pub fn new(dir: ShelfPath) -> Self {
        Self {
            dir,
            items: Vec::new(),
            next: 1,
        }
    }

    /// The folder files are saved into.
    pub fn dir(&self) -> &ShelfPath {
        &self.dir
    }

    /// The items, newest first.
    pub fn items(&self) -> &[ShelfItem] {
        &self.items
    }

    /// One item by id.
    pub fn get(&self, id: DownloadId) -> Option<&ShelfItem> {
        self.items.iter().find(|item| item.id == id)
    }

    /// The path for `name` at `attempt`, joined to the folder. `/` is the separator the edge
    /// accepts on every platform it supports.
    pub(super) fn path_for(&self, name: &str, attempt: u32) -> ShelfPath {
        let dir = self.dir.as_str().trim_end_matches('/');
        ShelfPath(format!("{dir}/{}", candidate_name(name, attempt)))
    }

    /// The first attempt (from `from`) whose path no live item on the shelf holds.
    pub(super) fn free_attempt(&self, name: &str, from: u32) -> u32 {
        (from..)
            .find(|&attempt| {
                let path = self.path_for(name, attempt);
                !self.items.iter().any(|item| item.path == path)
            })
            .unwrap_or(from)
    }

    pub(super) fn id_and_next(self) -> (DownloadId, Self) {
        let id = DownloadId(self.next);
        let next = self.next.saturating_add(1);
        (id, Self { next, ..self })
    }
}
