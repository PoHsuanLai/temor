//! The kitty file transfer protocol (OSC 5113): the wire format, the pure machine that receives
//! files and directories from the remote side, and the writer that applies its effects. Consent
//! is an input to the machine, never decided here.
//!
//! # temor's variants of the protocol
//!
//! Two small additions keep `temor get` and `temor save` quick and sure. Neither changes how a
//! kitty terminal and a kitty program talk to each other.
//!
//! - **`STARTED` before the question.** A terminal answers a `send` offer with `ac=status`
//!   `STARTED` for the offer's id at once, then asks the person, then answers `OK` or a failure.
//!   The sender can tell within a moment that the terminal takes part (`temor save` waits about
//!   three seconds for the first answer, then as long as the person needs).
//! - **`ac=receive` in temor's form.** kitty's own `receive` lists several paths in `file`
//!   frames. temor's is one frame: `ac=receive;id=X[;n=<name>]`, where `n` is the base64 name of
//!   a file on the terminal's downloads shelf (the newest file when it is left out). The
//!   terminal answers by offering that file with `ac=send` under the same id ([`serve`] and a
//!   [`SendSession`]), and the program takes it with a [`ReceiveSession`] ([`drive_receive`]).
//!   Nothing on the shelf, or a refusal, is an `ac=status` failure for the request's id. This is
//!   for temor talking to temor; another terminal answers it with silence or an error.

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
    Action, Compression, ErrorCode, FileId, FileType, FirstAnswer, KittyId, Message, Mode, Reply,
    ReplyStatus, Transmission, UnixNanos, WriteKind,
};
pub use receive::{Effect, EndWhy, Input, LinkPolicy, ReceiveSession, SavedFile, WriteOp};
pub use send::{
    ChunkEnd, Offered, SendEffect, SendEnd, SendFile, SendInput, SendSession, Served, describe,
    read_chunk, serve,
};
pub use wire::{encode_message, encode_reply, parse_message, parse_reply, readable_header};
pub use words::{REFUSED_BY_PERSON, UnknownWord};
