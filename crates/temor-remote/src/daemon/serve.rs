//! One connection: the greeting, then requests until the other end goes away.

use std::io::Read;
use std::sync::Arc;
use std::sync::mpsc::sync_channel;

use temor_wire::frame::FrameLimit;
use temor_wire::hello::{Negotiation, negotiate};
use temor_wire::ids::RequestId;
use temor_wire::message::{Event, Frame, Incoming, Message, RemoteError, Request};

use super::core::Daemon;
use super::dispatch::Done;
use super::files_worker::{FileWorker, Job, is_file_request};
use super::link::{Link, LinkId, Outgoing, QUEUE, run_writer};
use crate::error::Error;
use crate::protocol::{DAEMON_CAPS, hello};
use crate::transport::FrameReader;

impl Daemon {
    /// Serves one connection until it ends. `reader` and `writer` are the two directions of an
    /// ssh channel's stdio or of the daemon's local socket. Sessions started here outlive the
    /// connection: when it ends they are detached, not closed.
    ///
    /// Blocks; run it on a thread of its own. A greeting that cannot be agreed, or a stream that
    /// breaks, is an error; a peer that simply closes is `Ok`.
    pub fn serve<R, W>(self: &Arc<Self>, reader: R, writer: W) -> Result<(), Error>
    where
        R: Read,
        W: Outgoing,
    {
        let limit = FrameLimit::DEFAULT;
        let (frames_in, frames_out) = sync_channel(QUEUE);
        let id = LinkId(
            self.next_link
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                + 1,
        );
        let link = Arc::new(Link::new(id, frames_in, limit));
        let hung_up = link.hang_up_flag();
        std::thread::Builder::new()
            .name(format!("temor-remote-link-{}", id.0))
            .spawn(move || run_writer(frames_out, writer, hung_up))
            .map_err(|error| Error::io("starting the connection's writer", error))?;

        let ours = hello(self.config.helper, &DAEMON_CAPS);
        link.send(&Message::Hello(ours.clone()), None)?;
        let mut reader = FrameReader::new(reader, limit);
        let theirs = match reader.next_frame()? {
            Some(Incoming::Frame(Frame {
                message: Message::Hello(theirs),
                ..
            })) => theirs,
            _ => return Err(Error::NoGreeting),
        };
        match negotiate(&ours, &theirs) {
            Negotiation::Agreed(agreement) => link.agree(agreement.caps),
            Negotiation::Incompatible { reason } => return Err(Error::Incompatible(reason)),
        }

        self.lock_links().push(Arc::clone(&link));
        let result = self.serve_requests(&link, &mut reader);
        self.drop_link(&link);
        result
    }

    fn serve_requests<R: Read>(
        self: &Arc<Self>,
        link: &Arc<Link>,
        reader: &mut FrameReader<R>,
    ) -> Result<(), Error> {
        self.tell_ports(link)?;
        let files = FileWorker::start(self, link)?;
        loop {
            match reader.next_frame()? {
                None => return Ok(()),
                Some(Incoming::Frame(Frame {
                    message: Message::Request { id, body },
                    payload,
                })) if is_file_request(&body) => files.submit(Job { id, body, payload })?,
                Some(Incoming::Frame(Frame {
                    message: Message::Request { id, body },
                    payload,
                })) => self.answer(link, id, body, payload)?,
                Some(Incoming::UnknownRequest { id }) => {
                    link.send(&Message::unknown_request(id), None)?;
                }
                Some(Incoming::UnreadableRequest { id }) => {
                    link.send(&Message::unreadable_request(id), None)?;
                }
                // Greetings, answers and events from a client, and kinds this build does not
                // know, change nothing.
                Some(_) => {}
            }
        }
    }

    /// The ports known now go to a new connection that can use them, so it need not wait for the
    /// next change.
    fn tell_ports(&self, link: &Arc<Link>) -> Result<(), Error> {
        if !link.has(temor_wire::hello::Capability::Ports) {
            return Ok(());
        }
        let known = self
            .ports
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        if known.is_empty() {
            return Ok(());
        }
        link.send(&Message::Event(Event::Ports(known)), None)
    }

    pub(super) fn answer(
        self: &Arc<Self>,
        link: &Arc<Link>,
        id: RequestId,
        body: Request,
        payload: Option<Vec<u8>>,
    ) -> Result<(), Error> {
        let outcome = match self.handle(link, id, body, payload.as_deref()) {
            Ok(Done::Sent) => return Ok(()),
            Ok(Done::Reply(reply)) => return self.respond(link, id, Ok(reply), None),
            Ok(Done::Payload(reply, bytes)) => {
                return self.respond(link, id, Ok(reply), Some(&bytes));
            }
            Err(error) => Err(error),
        };
        self.respond(link, id, outcome, None)
    }

    /// Sends a response. One that cannot be encoded (over the frame limit) becomes an error
    /// response rather than ending the connection.
    fn respond(
        &self,
        link: &Arc<Link>,
        id: RequestId,
        outcome: Result<temor_wire::message::Reply, RemoteError>,
        payload: Option<&[u8]>,
    ) -> Result<(), Error> {
        match link.send(&Message::Response { id, outcome }, payload) {
            Err(Error::Wire(error)) => link.send(
                &Message::Response {
                    id,
                    outcome: Err(RemoteError::Io {
                        detail: error.to_string(),
                    }),
                },
                None,
            ),
            other => other,
        }
    }

    /// The connection has ended: its sessions stay, detached, and its watches go.
    fn drop_link(&self, link: &Arc<Link>) {
        self.lock_links().retain(|held| held.id != link.id);
        for session in self.registry.all() {
            session.detach(Some(link.id));
        }
        link.hang_up();
    }
}
