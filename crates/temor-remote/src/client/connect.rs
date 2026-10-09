//! Connecting: the greeting, the agreement, and the thread that reads what comes back.

use std::fmt;
use std::io::{Read, Write};
use std::sync::atomic::AtomicU64;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use temor_wire::frame::FrameLimit;
use temor_wire::hello::{Agreement, Hello, Negotiation, negotiate};
use temor_wire::message::{Frame, Incoming, Message};

use super::events::ClientEvent;
use super::pending::{Answer, Pending};
use crate::error::Error;
use crate::transport::{FrameReader, FrameWriter};

/// A connection to a helper. `Send` and `Sync`: requests from several threads take turns on the
/// link. What the helper says unasked arrives on the receiver [`Client::connect`] returns.
pub struct Client {
    pub(super) writer: Mutex<FrameWriter<Box<dyn Write + Send>>>,
    pub(super) pending: Arc<Pending>,
    pub(super) next: AtomicU64,
    pub(super) agreement: Agreement,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("agreement", &self.agreement)
            .finish_non_exhaustive()
    }
}

impl Client {
    /// Greets the helper over `reader` and `writer` (the ssh process's stdout and stdin) and
    /// agrees a protocol with it. `ours` is this side's greeting (see [`crate::hello`]). The
    /// helper's identity is in [`Client::agreement`]: an old helper is for the caller to update,
    /// not refused here.
    ///
    /// Blocks until the helper's greeting arrives, then starts a thread that reads the rest. The
    /// receiver carries what the helper tells us unasked; the last event after the link ends is
    /// [`ClientEvent::Closed`].
    pub fn connect<R, W>(
        reader: R,
        writer: W,
        ours: Hello,
    ) -> Result<(Self, Receiver<ClientEvent>), Error>
    where
        R: Read + Send + 'static,
        W: Write + Send + 'static,
    {
        let limit = FrameLimit::DEFAULT;
        let mut writer = FrameWriter::new(Box::new(writer) as Box<dyn Write + Send>, limit);
        writer.send(&Message::Hello(ours.clone()), None)?;
        let mut reader = FrameReader::new(reader, limit);
        let theirs = match reader.next()? {
            Some(Incoming::Frame(Frame {
                message: Message::Hello(theirs),
                ..
            })) => theirs,
            _ => return Err(Error::NoGreeting),
        };
        let agreement = match negotiate(&ours, &theirs) {
            Negotiation::Agreed(agreement) => agreement,
            Negotiation::Incompatible { reason } => return Err(Error::Incompatible(reason)),
        };
        let pending = Arc::new(Pending::new());
        let (events_in, events) = channel();
        let reading = Arc::clone(&pending);
        std::thread::Builder::new()
            .name("temor-remote-client".to_string())
            .spawn(move || read_loop(reader, &reading, &events_in))
            .map_err(|error| Error::io("starting the client's reader", error))?;
        let client = Self {
            writer: Mutex::new(writer),
            pending,
            next: AtomicU64::new(0),
            agreement,
        };
        Ok((client, events))
    }

    /// What the two ends agreed: the protocol, the helper's identity and the shared capabilities.
    pub fn agreement(&self) -> &Agreement {
        &self.agreement
    }
}

fn read_loop<R: Read>(mut reader: FrameReader<R>, pending: &Pending, events: &Sender<ClientEvent>) {
    loop {
        match reader.next() {
            Ok(Some(Incoming::Frame(Frame { message, payload }))) => match message {
                Message::Response { id, outcome } => {
                    pending.complete(id, Ok(Answer { outcome, payload }));
                }
                Message::Event(event) => {
                    if let Some(event) = ClientEvent::from_wire(event, payload) {
                        // Nobody listening any more is the window having closed the link.
                        let _ = events.send(event);
                    }
                }
                _ => {}
            },
            Ok(Some(Incoming::UnknownResponse { id })) => {
                pending.complete(id, Err(Error::UnexpectedReply));
            }
            Ok(Some(Incoming::UnreadableEvent { kind })) => {
                let _ = events.send(ClientEvent::Unreadable { kind });
            }
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => break,
        }
    }
    pending.close();
    let _ = events.send(ClientEvent::Closed);
}
