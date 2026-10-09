//! Framing: a message, its JSON and an optional binary payload, cut out of a byte stream.
//!
//! ```text
//! u32 LE json length | u32 LE payload length | JSON | payload
//! ```
//!
//! A payload length of `u32::MAX` means the frame has no payload. A message that carries bytes
//! always has a payload, even an empty one, so the two cases stay apart.

mod decoder;
mod encode;
mod header;
mod limit;

#[cfg(test)]
mod tests;

pub use decoder::Decoder;
pub use encode::encode;
pub use limit::FrameLimit;
