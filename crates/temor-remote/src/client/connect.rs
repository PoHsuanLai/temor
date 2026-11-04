//! Connecting: the greeting, the agreement, and the thread that reads what comes back.

use std::fmt;
use std::io::{Read, Write};
use std::sync::atomic::AtomicU64;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use temor_wire::frame::FrameLimit;
use temor_wire::hello::{Agreement, Hello};
use temor_wire::ids::RequestId;
use temor_wire::message::{Frame, Incoming, Message};

use super::events::ClientEvent;
use super::pending::{Answer, Pending};
use crate::error::Error;
use crate::transport::{FrameReader, FrameWriter, greet};

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
        let mut reader = FrameReader::new(reader, limit);
        let agreement = greet(&ours, |hello| writer.send(hello, None), &mut reader)?;
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

/// Where one frame from the helper goes.
#[derive(Debug)]
enum Route {
    /// The answer to the call `RequestId`.
    Answer(RequestId, Result<Answer, Error>),
    /// To the window, as something the helper said unasked.
    Event(ClientEvent),
    Nowhere,
}

fn route(incoming: Incoming) -> Route {
    match incoming {
        Incoming::Frame(Frame {
            message: Message::Response { id, outcome },
            payload,
        }) => Route::Answer(id, Ok(Answer { outcome, payload })),
        Incoming::Frame(Frame {
            message: Message::Event(event),
            payload,
        }) => ClientEvent::from_wire(event, payload).map_or(Route::Nowhere, Route::Event),
        Incoming::UnknownResponse { id } => Route::Answer(id, Err(Error::UnexpectedReply)),
        Incoming::UnreadableEvent { kind } => Route::Event(ClientEvent::Unreadable { kind }),
        // Requests, greetings and kinds this build does not know.
        _ => Route::Nowhere,
    }
}

fn read_loop<R: Read>(mut reader: FrameReader<R>, pending: &Pending, events: &Sender<ClientEvent>) {
    while let Ok(Some(incoming)) = reader.next_frame() {
        match route(incoming) {
            Route::Answer(id, answer) => pending.complete(id, answer),
            // Nobody listening any more is the window having closed the link.
            Route::Event(event) => drop(events.send(event)),
            Route::Nowhere => {}
        }
    }
    pending.close();
    let _ = events.send(ClientEvent::Closed);
}

#[cfg(test)]
mod tests {
    use super::*;
    use temor_wire::ids::RemoteSessionId;
    use temor_wire::message::{Event, Reply, Request};

    #[test]
    fn answers_go_to_their_calls_events_to_the_window_and_the_rest_nowhere() {
        let frame = |message| {
            Incoming::Frame(Frame {
                message,
                payload: Some(b"x".to_vec()),
            })
        };
        let id = RequestId(7);
        let session = RemoteSessionId(3);
        match route(frame(Message::Response {
            id,
            outcome: Ok(Reply::Ack),
        })) {
            Route::Answer(to, Ok(answer)) => {
                assert_eq!((to, answer.payload), (id, Some(b"x".to_vec())));
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            route(Incoming::UnknownResponse { id }),
            Route::Answer(to, Err(Error::UnexpectedReply)) if to == id
        ));
        assert!(matches!(
            route(frame(Message::Event(Event::Output { session }))),
            Route::Event(ClientEvent::Output { bytes, .. }) if bytes == b"x"
        ));
        assert!(matches!(
            route(Incoming::UnreadableEvent {
                kind: "Exited".into()
            }),
            Route::Event(ClientEvent::Unreadable { .. })
        ));
        assert!(matches!(
            route(frame(Message::Request {
                id,
                body: Request::Ping
            })),
            Route::Nowhere
        ));
        assert!(matches!(route(Incoming::Ignored), Route::Nowhere));
    }
}
