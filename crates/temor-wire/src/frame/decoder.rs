//! Cutting frames out of bytes as they arrive.

use super::HEADER;
use super::limit::{FrameLimit, header_lengths};
use crate::WireError;
use serde::de::DeserializeOwned;

/// A decoded message and the payload that came with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame<T> {
    /// The message.
    pub message: T,
    /// The payload; empty when the message has none.
    pub payload: Vec<u8>,
}

/// Whether the decoder can still take bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Health {
    Sound,
    Spent,
}

/// Cuts frames out of bytes as they arrive. Push whatever was read, then pull frames until
/// `None`. A frame over the limit, or JSON that is not the expected message, ends the stream:
/// that error is returned once, and every call after it is [`WireError::Spent`].
#[derive(Debug, Clone)]
pub struct Decoder {
    limit: FrameLimit,
    buffer: Vec<u8>,
    /// Where the next unread frame starts in `buffer`.
    start: usize,
    health: Health,
}

impl Decoder {
    /// A decoder with nothing buffered that refuses frames over `limit`.
    pub fn new(limit: FrameLimit) -> Self {
        Self {
            limit,
            buffer: Vec::new(),
            start: 0,
            health: Health::Sound,
        }
    }

    /// Adds bytes that arrived.
    pub fn push(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    /// The next whole frame, or `None` until more bytes arrive.
    pub fn next_frame<T: DeserializeOwned>(&mut self) -> Result<Option<Frame<T>>, WireError> {
        if self.health == Health::Spent {
            return Err(WireError::Spent);
        }
        let result = self.take_frame();
        if result.is_err() {
            self.health = Health::Spent;
        }
        result
    }

    /// Ends the stream. Bytes of an unfinished frame mean it was cut off.
    pub fn finish(self) -> Result<(), WireError> {
        if self.start < self.buffer.len() {
            Err(WireError::Truncated)
        } else {
            Ok(())
        }
    }

    fn take_frame<T: DeserializeOwned>(&mut self) -> Result<Option<Frame<T>>, WireError> {
        let pending = self.buffer.get(self.start..).unwrap_or_default();
        let Some(header) = pending.first_chunk::<HEADER>() else {
            return Ok(None);
        };
        // The header is judged before any body is waited for, so an announced length over the
        // limit is refused at once and never buffered.
        let (json_len, payload_len) = header_lengths(header, self.limit)?;
        let total = HEADER + json_len + payload_len;
        let Some(whole) = pending.get(..total) else {
            return Ok(None);
        };
        let json = whole.get(HEADER..HEADER + json_len).unwrap_or_default();
        let message = serde_json::from_slice(json).map_err(|error| WireError::Malformed {
            reason: error.to_string(),
        })?;
        let payload = whole.get(HEADER + json_len..).unwrap_or_default().to_vec();
        self.start += total;
        self.compact();
        Ok(Some(Frame { message, payload }))
    }

    /// Drops the bytes of frames already taken once they make up half the buffer, so the copy is
    /// paid for by the frames it moves.
    fn compact(&mut self) {
        if self.start == self.buffer.len() {
            self.buffer.clear();
            self.start = 0;
        } else if self.start >= self.buffer.len() / 2 {
            self.buffer.drain(..self.start);
            self.start = 0;
        }
    }
}
