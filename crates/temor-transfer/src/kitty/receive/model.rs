//! The receive machine's types. Writes are described here and performed at the edge.
//!
//! The edge contract: every `Effect::Write` is performed in order, and each one yields exactly
//! one result. A success needs no input. A failure is fed back as `Input::WriteFailed`, which
//! cancels that file, or the whole transfer when the failure is not for an open file.

use std::collections::{BTreeMap, BTreeSet};

use crate::consent::Consent;
use crate::kitty::model::{ErrorCode, FileId, KittyId, Message, Reply, UnixNanos, WriteKind};
use crate::name::RelPath;
use crate::origin::Origin;
use crate::units::{Bytes, Limits};

/// What the machine is told.
///
/// The edge contract: every `Effect::Write` is performed in order. A failure is always fed back
/// as `WriteFailed`. A successful `Finish` is fed back as `WriteDone`, because a file counts as
/// saved only once its `Finish` has succeeded. Other successes need no input.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Input {
    Frame(Message),
    /// A frame that did not parse, whose action and id were still readable (see
    /// `readable_header`). It is answered with `EINVAL` for its transfer.
    Malformed {
        id: KittyId,
    },
    Decided(Consent),
    /// The stream was dropped. Partly written files are discarded and the transfer ends.
    Aborted,
    /// An `Effect::Write` failed. `fid` names the file it was for, when there was one, and `op`
    /// says which operation failed.
    WriteFailed {
        fid: Option<FileId>,
        op: WriteKind,
        error: ErrorCode,
    },
    /// The `Finish` for `fid` succeeded.
    WriteDone {
        fid: FileId,
    },
}

/// A file-system change the edge must make, in order. Paths are relative to the transfer's root.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WriteOp {
    MakeDir(RelPath),
    /// Create a new, empty file. It must not exist yet.
    CreateFile(RelPath),
    Append {
        path: RelPath,
        data: Vec<u8>,
    },
    /// Set the modification time, when the remote sent one.
    Finish {
        path: RelPath,
        modified: Option<UnixNanos>,
    },
    /// Remove a partly written file after a failure or cancel.
    Discard(RelPath),
}

/// A regular file that completed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedFile {
    pub path: RelPath,
    pub size: Bytes,
}

/// How a session ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EndWhy {
    Finished,
    Refused,
    Cancelled,
    Failed,
    Aborted,
}

/// What the edge must do after a step.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Effect {
    /// Ask the person whether to accept an offer from this origin. Answer with `Input::Decided`.
    AskConsent(Origin),
    /// Send this status back to the remote (encode with `encode_reply`).
    Reply(Reply),
    Write(WriteOp),
    Saved(SavedFile),
    Ended(EndWhy),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Phase {
    Idle,
    Asking(KittyId),
    Receiving(KittyId),
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenFile {
    pub(crate) path: RelPath,
    pub(crate) expected: Option<Bytes>,
    pub(crate) written: u64,
    pub(crate) modified: Option<UnixNanos>,
}

/// One transfer from one origin. Create a new session for each offer.
#[derive(Debug, Clone)]
pub struct ReceiveSession {
    pub(crate) origin: Origin,
    pub(crate) limits: Limits,
    pub(crate) phase: Phase,
    pub(crate) dirs: BTreeMap<FileId, RelPath>,
    pub(crate) open: BTreeMap<FileId, OpenFile>,
    pub(crate) received: u64,
    pub(crate) entries: u64,
    /// Every path this transfer has created or claimed, so a second entry with the same name is
    /// refused instead of clobbering the first.
    pub(crate) claimed: BTreeSet<RelPath>,
    /// Files whose data is complete and whose `Finish` is in flight.
    pub(crate) finishing: BTreeMap<FileId, OpenFile>,
}

impl ReceiveSession {
    pub fn new(origin: Origin, limits: Limits) -> Self {
        Self {
            origin,
            limits,
            phase: Phase::Idle,
            dirs: BTreeMap::new(),
            open: BTreeMap::new(),
            received: 0,
            entries: 0,
            claimed: BTreeSet::new(),
            finishing: BTreeMap::new(),
        }
    }

    /// The step: `(session, input) -> (session, effects)`. Pure; the edge carries out the effects.
    pub fn step(self, input: Input) -> (Self, Vec<Effect>) {
        super::step::step(self, input)
    }
}
