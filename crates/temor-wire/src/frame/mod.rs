//! Framing: a message, its JSON and an optional binary payload, cut out of a byte stream.
//!
//! ```text
//! u32 LE json length | u32 LE payload length | JSON | payload
//! ```

mod decoder;
mod encode;
mod limit;

#[cfg(test)]
mod tests;

pub use decoder::{Decoder, Frame};
pub use encode::encode;
pub use limit::FrameLimit;

/// Bytes in a frame header: two `u32` lengths.
pub(crate) const HEADER: usize = 8;
