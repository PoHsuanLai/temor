//! What a pane tells the window about transfers, and what it is set up with.

use std::path::PathBuf;

use temor_transfer::Limits;

/// One offer or request, numbered within its pane. The window answers it by this number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OfferId(pub u64);

/// What a pane needs to take files: a folder of its own to stage them in, and the caps.
#[derive(Debug, Clone)]
pub struct TransferSetup {
    /// A folder that belongs to this pane. Each transfer gets a fresh folder inside it; the
    /// window moves the files out and the pane never reads them again.
    pub staging: PathBuf,
    pub limits: Limits,
}

/// What a program offers.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum OfferKind {
    /// Files by kitty's protocol. Their names come after the person has said yes.
    Files,
    /// One file by iTerm2's `File=`, which arrived whole. Nothing is kept until the person
    /// says yes.
    Inline { name: String, size: u64 },
}

pub use temor_transfer::kitty::TransferEnd;

/// One thing a transfer did. `file` names a file within its offer (its path in the transfer).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TransferEvent {
    /// A program offers files. Answer with `PaneHandle::decide`.
    Offered { offer: OfferId, kind: OfferKind },
    /// A program asks for a file from the shelf (`name`, or the newest). Answer with
    /// `PaneHandle::serve`.
    Wanted {
        offer: OfferId,
        name: Option<String>,
    },
    /// A file begins arriving.
    Started {
        offer: OfferId,
        file: String,
        name: String,
        size: Option<u64>,
    },
    /// Some more of a file arrived.
    Progress {
        offer: OfferId,
        file: String,
        received: u64,
    },
    /// A file is complete in the staging folder at `path`.
    Saved {
        offer: OfferId,
        file: String,
        path: PathBuf,
        size: u64,
    },
    /// The transfer is over.
    Ended { offer: OfferId, how: TransferEnd },
    /// Something the program sent could not be read; `why` is for the log.
    Unreadable { why: String },
}
