//! The kitty file transfer protocol (OSC 5113): the wire format, the pure machine that receives
//! files and directories from the remote side, and the writer that applies its effects. Consent
//! is an input to the machine, never decided here.

mod io;
#[macro_use]
mod words;
mod model;
mod receive;
mod wire;

pub use io::{apply_write, wire_code};
pub use model::{
    Action, Compression, ErrorCode, FileId, FileType, KittyId, Message, Mode, Reply, ReplyStatus,
    Transmission, UnixNanos, WriteKind,
};
pub use receive::{Effect, EndWhy, Input, ReceiveSession, SavedFile, WriteOp};
pub use wire::{encode_message, encode_reply, parse_message, readable_header};
pub use words::UnknownWord;
