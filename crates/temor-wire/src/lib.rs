//! temor's wire protocol (PLAN.md §9b, Tier B): the versioned conversation between temor and
//! `temor-remote` over an ssh channel on stdio.
//!
//! The crate is pure. It turns bytes into frames and frames into messages, decides whether two
//! ends can speak, describes every request, reply and event as data, and steps the reconnect
//! machine. It never reads a socket, never spawns and never sleeps: the caller feeds bytes in,
//! passes the clock in, and carries out the effects it gets back.
//!
//! Framing follows bayonet's wire (`u32 LE json length | u32 LE payload length | JSON | payload`),
//! so bulk bytes (terminal snapshots, file contents, pty output) cross without encoding.
//! Messages are JSON through `serde_json`: the protocol is versioned by hand, the frames stay
//! readable in a log, and the payload carries everything binary.
//!
//! Layout: [`frame`] (framing), [`hello`] (the handshake and version negotiation), [`message`]
//! (requests, replies, events and errors), [`link`] (the reconnect step machine), with the
//! vocabulary in [`ids`], [`units`] and [`digest`]. Every crate-level failure is [`WireError`].
//!
//! # Example
//!
//! One message, encoded to a frame and read back by the decoder:
//!
//! ```
//! use temor_wire::frame::{Decoder, FrameLimit, encode};
//! use temor_wire::ids::RequestId;
//! use temor_wire::message::{Incoming, Message, Reply};
//!
//! # fn main() -> Result<(), temor_wire::WireError> {
//! let limit = FrameLimit(1 << 20);
//! let sent = Message::Response { id: RequestId(7), outcome: Ok(Reply::Ack) };
//! let bytes = encode(&sent, None, limit)?;
//!
//! let mut decoder = Decoder::new(limit);
//! decoder.push(&bytes);
//! let Some(Incoming::Frame(frame)) = decoder.next_message()? else {
//!     panic!("a whole frame was pushed");
//! };
//! assert_eq!(frame.message, sent);
//! # Ok(())
//! # }
//! ```

pub mod digest;
pub mod error;
pub mod frame;
pub mod hello;
pub mod ids;
pub mod link;
pub mod message;
pub mod units;

pub use error::WireError;
