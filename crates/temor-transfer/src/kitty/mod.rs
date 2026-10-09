//! The kitty file transfer protocol (OSC 5113): the wire format, the pure machine that receives
//! files and directories from the remote side, and the writer that applies its effects. Consent
//! is an input to the machine, never decided here.

mod drive;
mod frames;
mod io;
#[macro_use]
mod words;
mod model;
mod receive;
mod send;
mod wire;

pub use drive::{Link, Waits, drive_receive, drive_send};
pub use frames::FrameScanner;
pub use io::{apply_write, wire_code};
pub use model::{
    Action, Compression, ErrorCode, FileId, FileType, KittyId, Message, Mode, Reply, ReplyStatus,
    Transmission, UnixNanos, WriteKind,
};
pub use receive::{Effect, EndWhy, Input, ReceiveSession, SavedFile, WriteOp};
pub use send::{
    ChunkEnd, Offered, SendEffect, SendEnd, SendFile, SendInput, SendSession, Served, describe,
    read_chunk, serve,
};
pub use wire::{encode_message, encode_reply, parse_message, parse_reply, readable_header};
pub use words::UnknownWord;
