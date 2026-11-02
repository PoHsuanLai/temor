//! One connection: the greeting, then requests until the other end goes away.

use std::io::Read;
use std::sync::Arc;
use std::sync::mpsc::sync_channel;

use temor_wire::frame::FrameLimit;
use temor_wire::ids::RequestId;
use temor_wire::message::{Event, Frame, Incoming, Message, RemoteError, Request};

use super::core::Daemon;
use super::dispatch::Done;
use super::files_worker::{FileWorker, Job, is_file_request};
use super::link::{Link, LinkId, Outgoing, QUEUE, run_writer};
use super::session::Command;
use crate::error::Error;
use crate::protocol::{DAEMON_CAPS, hello};
use crate::sync::lock;
use crate::transport::{FrameReader, greet};

/// Where one frame from a client goes.
#[derive(Debug)]
enum Route {
    /// To the connection's file worker, so a slow file never holds up the rest.
    Files(Job),
    /// Answered on the reader thread.
    Here(Job),
    /// Answered with this refusal.
    Refuse(Message),
    Nowhere,
}

fn route_request(incoming: Incoming) -> Route {
    match incoming {
        Incoming::Frame(Frame {
            message: Message::Request { id, body },
            payload,
        }) => match is_file_request(&body) {
            true => Route::Files(Job { id, body, payload }),
            false => Route::Here(Job { id, body, payload }),
        },
        Incoming::UnknownRequest { id } => Route::Refuse(Message::unknown_request(id)),
        Incoming::UnreadableRequest { id } => Route::Refuse(Message::unreadable_request(id)),
        // Greetings, answers and events from a client, and kinds this build does not know,
        // change nothing.
        _ => Route::Nowhere,
    }
}

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
        let mut reader = FrameReader::new(reader, limit);
        link.agree(greet(&ours, |hello| link.send(hello, None), &mut reader)?.caps);

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
        while let Some(incoming) = reader.next_frame()? {
            match route_request(incoming) {
                Route::Files(job) => files.submit(job)?,
                Route::Here(job) => self.answer(link, job.id, job.body, job.payload)?,
                Route::Refuse(refusal) => link.send(&refusal, None)?,
                Route::Nowhere => {}
            }
        }
        Ok(())
    }

    /// The ports known now go to a new connection that can use them, so it need not wait for the
    /// next change.
    fn tell_ports(&self, link: &Arc<Link>) -> Result<(), Error> {
        if !link.has(temor_wire::hello::Capability::Ports) {
            return Ok(());
        }
        let known = lock(&self.ports).clone();
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
            session.send(Command::Detach(link.id));
        }
        link.hang_up();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use temor_wire::message::RemotePath;

    fn request(body: Request) -> Incoming {
        Incoming::Frame(Frame {
            message: Message::Request {
                id: RequestId(1),
                body,
            },
            payload: None,
        })
    }

    #[test]
    fn file_requests_go_to_the_worker_the_rest_are_answered_here_and_unknown_ones_refused() {
        let path = RemotePath("/tmp/a".into());
        assert!(matches!(
            route_request(request(Request::Stat { path })),
            Route::Files(_)
        ));
        assert!(matches!(
            route_request(request(Request::Ping)),
            Route::Here(_)
        ));
        assert!(matches!(
            route_request(Incoming::UnknownRequest { id: RequestId(2) }),
            Route::Refuse(refusal) if refusal == Message::unknown_request(RequestId(2))
        ));
        assert!(matches!(
            route_request(Incoming::UnreadableRequest { id: RequestId(3) }),
            Route::Refuse(refusal) if refusal == Message::unreadable_request(RequestId(3))
        ));
        assert!(matches!(route_request(Incoming::Ignored), Route::Nowhere));
    }
}
