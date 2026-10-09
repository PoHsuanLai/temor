//! The receive machine's types. Writes are described here and performed at the edge.

use std::collections::BTreeMap;

use ds_core::word::Word;

use crate::kitty::model::{FileId, KittyId, Message, Reply, UnixNanos};
use crate::name::RelPath;
use crate::origin::Origin;
use crate::units::{Bytes, Limits};

/// The person's answer to a remote offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum Consent {
    Allow,
    Refuse,
}

/// What the machine is told: a frame from the remote, or the person's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    Frame(Message),
    Decided(Consent),
}

/// A file-system change the edge must make, in order. Paths are relative to the transfer's root.
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum EndWhy {
    Finished,
    Refused,
    Cancelled,
    Failed,
}

/// What the edge must do after a step.
#[derive(Debug, Clone, PartialEq, Eq)]
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
        }
    }

    /// The step: `(session, input) -> (session, effects)`. Pure; the edge carries out the effects.
    pub fn step(self, input: Input) -> (Self, Vec<Effect>) {
        super::step::step(self, input)
    }
}
