//! The downloads shelf (§9): what arrived, from where, and how far it got.

use crate::vocab::ids::{BlockId, Bytes, DownloadId, TabId};
use crate::vocab::text::{HostName, ShelfPath};
use ds_core::time::stamp::Stamp;
use serde::{Deserialize, Serialize};

/// One item on the shelf.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Download {
    /// Its identity.
    pub id: DownloadId,
    /// Where it is saved.
    pub path: ShelfPath,
    /// Where it came from, for "Show in its tab".
    pub from: Origin,
    /// Its size, or what has arrived so far.
    pub size: Bytes,
    /// When it arrived.
    pub at: Stamp,
    /// How far it got.
    pub state: Transfer,
}

/// Where a download came from.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum Origin {
    /// Sent by a remote host over a tab's terminal stream.
    Remote {
        /// The host.
        host: HostName,
        /// The tab.
        tab: TabId,
    },
    /// Sent over the terminal stream of a pane on this computer (a file transfer sequence in a
    /// program's output). Any file a program prints can carry one, so it asks like a host's.
    LocalStream {
        /// The tab.
        tab: TabId,
    },
    /// Saved by the person on this computer: "Save output", a screenshot, `temor save`. Never
    /// a sequence in a program's output; that is [`Origin::LocalStream`].
    Local {
        /// The tab.
        tab: TabId,
        /// The block it was saved from, if one.
        #[serde(default)]
        block: Option<BlockId>,
    },
    /// Piped in (`temor view -`).
    Pipe,
}

/// How far a transfer got.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum Transfer {
    /// Offered by a host; waiting for the person.
    Offered,
    /// Arriving.
    Receiving {
        /// What has arrived.
        received: Bytes,
    },
    /// Complete and saved.
    Saved,
    /// Turned down.
    Refused,
    /// Broken off part way.
    Broken,
}
