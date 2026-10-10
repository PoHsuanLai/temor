//! Frames over a byte stream: the glue between `temor-wire`'s pure decoder and encoder and a
//! reader or writer. Blocking; the caller owns the threads.

use std::io::{self, Read, Write};

use temor_wire::frame::{Decoder, FrameLimit, encode};
use temor_wire::message::{Incoming, Message};

use crate::error::Error;

/// Reads frames from a stream. Not `Sync`; move it to the thread that reads.
#[derive(Debug)]
pub struct FrameReader<R> {
    inner: R,
    decoder: Option<Decoder>,
    buffer: Vec<u8>,
}

impl<R: Read> FrameReader<R> {
    /// A reader that refuses frames over `limit`.
    pub fn new(inner: R, limit: FrameLimit) -> Self {
        Self {
            inner,
            decoder: Some(Decoder::new(limit)),
            buffer: vec![0; 64 * 1024],
        }
    }

    /// The next frame, blocking until it is whole. `None` when the stream ended between frames;
    /// a stream cut inside a frame is an error.
    pub fn next_frame(&mut self) -> Result<Option<Incoming>, Error> {
        loop {
            let Some(decoder) = self.decoder.as_mut() else {
                return Ok(None);
            };
            if let Some(incoming) = decoder.next_message()? {
                return Ok(Some(incoming));
            }
            let read = read_some(&mut self.inner, &mut self.buffer)?;
            if read == 0 {
                if let Some(decoder) = self.decoder.take() {
                    decoder.finish()?;
                }
                return Ok(None);
            }
            if let Some(decoder) = self.decoder.as_mut() {
                decoder.push(&self.buffer[..read]);
            }
        }
    }
}

fn read_some(inner: &mut impl Read, buffer: &mut [u8]) -> Result<usize, Error> {
    loop {
        match inner.read(buffer) {
            Ok(read) => return Ok(read),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(Error::io("reading the link", error)),
        }
    }
}

/// Writes frames to a stream.
#[derive(Debug)]
pub struct FrameWriter<W> {
    inner: W,
    limit: FrameLimit,
}

impl<W: Write> FrameWriter<W> {
    /// A writer that refuses frames over `limit`.
    pub fn new(inner: W, limit: FrameLimit) -> Self {
        Self { inner, limit }
    }

    /// Encodes and writes one frame.
    pub fn send(&mut self, message: &Message, payload: Option<&[u8]>) -> Result<(), Error> {
        let bytes = encode(message, payload, self.limit)?;
        self.write_bytes(&bytes)
    }

    /// Writes bytes that are already a whole frame, and flushes.
    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.inner
            .write_all(bytes)
            .and_then(|()| self.inner.flush())
            .map_err(|error| Error::io("writing the link", error))
    }
}
