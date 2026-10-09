//! Cutting frames out of bytes as they arrive.

use super::header::{HEADER, Lengths, read, total};
use super::limit::FrameLimit;
use crate::WireError;
use crate::message::Incoming;
use ds_core::word::Word;

/// Whether the decoder can still take bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
enum Health {
    Sound,
    Spent,
}

/// Cuts frames out of bytes as they arrive. Push whatever was read, then pull frames until
/// `None`.
///
/// A frame that is over the limit, or whose JSON is not a message this decoder can read, ends the
/// stream. That error is returned once; every later call is [`WireError::Spent`], and
/// [`Decoder::finish`] returns the same error.
#[derive(Debug, Clone)]
pub struct Decoder {
    limit: FrameLimit,
    buffer: Vec<u8>,
    /// Where the next unread frame starts in `buffer`.
    start: usize,
    health: Health,
    failure: Option<WireError>,
}

impl Decoder {
    /// A decoder with nothing buffered that refuses frames over `limit`.
    pub fn new(limit: FrameLimit) -> Self {
        Self {
            limit,
            buffer: Vec::new(),
            start: 0,
            health: Health::Sound,
            failure: None,
        }
    }

    /// Adds bytes that arrived.
    pub fn push(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    /// The next whole frame, read as an [`Incoming`], or `None` until more bytes arrive.
    pub fn next_message(&mut self) -> Result<Option<Incoming>, WireError> {
        if self.health == Health::Spent {
            return Err(WireError::Spent);
        }
        match self.take() {
            Ok(incoming) => Ok(incoming),
            Err(error) => {
                self.health = Health::Spent;
                self.failure = Some(error.clone());
                Err(error)
            }
        }
    }

    /// Ends the stream. An earlier refusal is returned again; otherwise bytes of an unfinished
    /// frame mean it was cut off.
    pub fn finish(self) -> Result<(), WireError> {
        if let Some(error) = self.failure {
            Err(error)
        } else if self.start < self.buffer.len() {
            Err(WireError::Truncated)
        } else {
            Ok(())
        }
    }

    fn take(&mut self) -> Result<Option<Incoming>, WireError> {
        let pending = self.buffer.get(self.start..).unwrap_or_default();
        let Some(header) = pending.first_chunk::<HEADER>() else {
            return Ok(None);
        };
        // The header is judged before any body is waited for.
        let lengths: Lengths = read(header, self.limit)?;
        // The real size is reported when this target cannot address it, never a made-up one.
        let real = total(lengths);
        let Ok(size) = usize::try_from(real) else {
            return Err(self.limit.refuse(real));
        };
        let Some(whole) = pending.get(..size) else {
            return Ok(None);
        };
        let json_end = HEADER + lengths.json;
        let json = whole.get(HEADER..json_end).unwrap_or_default();
        let payload = lengths.payload.and_then(|_| whole.get(json_end..size));
        let incoming = Incoming::parse(json, payload)?;
        self.start += size;
        self.compact();
        Ok(Some(incoming))
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
