//! Frames over a byte stream: the glue between `temor-wire`'s pure decoder and encoder and a
//! reader or writer. Blocking; the caller owns the threads.

use std::io::{self, Read, Write};

use temor_wire::frame::{Decoder, FrameLimit, encode};
use temor_wire::hello::{Agreement, Hello, Negotiation, negotiate};
use temor_wire::message::{Frame, Incoming, Message};

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
            .map_err(|error| Error::LinkClosed(Some(error)))
    }
}

/// The greeting exchange both ends make: `send_hello` puts `ours` on the link, then the first
/// frame back must be the other end's greeting (else [`Error::NoGreeting`]) and the two must
/// share a protocol version (else [`Error::Incompatible`]).
pub fn greet<R: Read>(
    ours: &Hello,
    send_hello: impl FnOnce(&Message) -> Result<(), Error>,
    reader: &mut FrameReader<R>,
) -> Result<Agreement, Error> {
    send_hello(&Message::Hello(ours.clone()))?;
    let theirs = match reader.next_frame()? {
        Some(Incoming::Frame(Frame {
            message: Message::Hello(theirs),
            ..
        })) => theirs,
        _ => return Err(Error::NoGreeting),
    };
    match negotiate(ours, &theirs) {
        Negotiation::Agreed(agreement) => Ok(agreement),
        Negotiation::Incompatible { reason } => Err(Error::Incompatible(reason)),
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};

    use temor_wire::frame::FrameLimit;
    use temor_wire::ids::RequestId;
    use temor_wire::message::{Message, Request};

    use super::FrameWriter;
    use crate::error::Error;

    struct Broken;

    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_write_to_a_dead_link_reads_as_the_link_being_lost() {
        let mut writer = FrameWriter::new(Broken, FrameLimit::DEFAULT);
        let error = writer
            .send(
                &Message::Request {
                    id: RequestId(1),
                    body: Request::Ping,
                },
                None,
            )
            .expect_err("the pipe is broken");
        assert!(
            matches!(&error, Error::LinkClosed(Some(cause)) if cause.kind() == std::io::ErrorKind::BrokenPipe),
            "{error:?}"
        );
        assert!(error.is_link_lost());
    }
}
