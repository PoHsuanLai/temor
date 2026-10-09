//! Types of the sending machine. The machine never touches a file: it asks for each chunk with
//! `SendEffect::Read` and is given it back as `SendInput::Chunk`.

use crate::kitty::model::{KittyId, Message, Reply, UnixNanos};
use crate::name::FileName;
use crate::units::Bytes;

/// The most raw bytes one data frame carries (4096 characters of base64).
pub(super) const CHUNK: u64 = 3072;

/// One file to send, as announced to the receiver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendFile {
    pub name: FileName,
    pub size: Bytes,
    pub modified: Option<UnixNanos>,
}

/// Whether a chunk is the last of its file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChunkEnd {
    More,
    Last,
}

/// What the machine is told.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SendInput {
    /// A status from the receiver.
    Reply(Reply),
    /// The bytes asked for by `SendEffect::Read`.
    Chunk { data: Vec<u8>, end: ChunkEnd },
    /// The connection went away or the person gave up.
    Aborted,
}

/// How a send ended.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SendEnd {
    /// Every file arrived.
    Delivered,
    /// The receiver said no, with its reason.
    Refused { message: String },
    /// Something broke part way, with the receiver's reason.
    Failed { message: String },
    /// Stopped by the sender's side.
    Cancelled,
}

/// What the edge must do.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SendEffect {
    /// Write this frame (`encode_message`) to the other side.
    Emit(Message),
    /// Read up to `max` bytes of file number `file` (its place in the list given to `new`),
    /// continuing where the last read stopped, and answer with `SendInput::Chunk`.
    Read {
        file: usize,
        max: Bytes,
    },
    Ended(SendEnd),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Phase {
    /// The offer is out; waiting to hear it was accepted.
    Offered,
    /// Streaming file number `.0`.
    Streaming(usize),
    /// All data is out; waiting for the receiver to close the transfer.
    Finishing,
    Done,
}

/// One transfer of one or more files. Create one per transfer. `Send`; one thread drives it.
#[derive(Debug, Clone)]
pub struct SendSession {
    pub(super) id: KittyId,
    pub(super) files: Vec<SendFile>,
    pub(super) phase: Phase,
}
