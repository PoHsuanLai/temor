//! The requests, one typed method each. A capability the helper did not agree is refused here,
//! before anything is sent.

use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use temor_wire::hello::Capability;
use temor_wire::ids::{RequestId, SessionId, WatchId};
use temor_wire::message::{
    DirEntry, FileMeta, FileStamp, IdleLimit, ListeningPort, OpenSession, RemotePath, Reply,
    Request, SessionInfo, WriteCheck, WriteMode,
};
use temor_wire::units::{ByteCount, ByteRange, CellPx, Size};

use super::connect::Client;
use super::pending::Answer;
use crate::error::Error;

/// A request on the line whose answer has not been read yet.
#[derive(Debug)]
pub(super) struct InFlight {
    id: RequestId,
    waiting: Receiver<Result<Answer, Error>>,
}

impl InFlight {
    /// Stops waiting: the answer, when it comes, is dropped.
    pub(super) fn forget(self, client: &Client) {
        client.pending.forget(self.id);
    }
}

/// A session the client attached to: its size, and the snapshot of its screen. The live output
/// follows as [`ClientEvent::Output`](super::ClientEvent::Output) events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attached {
    /// The session.
    pub session: SessionId,
    /// Its size now.
    pub size: Size,
    /// libghostty's encoding of the screen, scrollback and modes.
    pub snapshot: Vec<u8>,
}

#[cfg(feature = "ghostty")]
impl Attached {
    /// The snapshot, ready for `temor_vt::Terminal::restore`.
    pub fn screen(&self) -> temor_vt::Snapshot {
        temor_vt::Snapshot::from_bytes(self.snapshot.clone())
    }
}

/// A span of a remote file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadChunk {
    /// The file's stamp, to check against on write-back.
    pub stamp: FileStamp,
    /// The file's full length.
    pub total: ByteCount,
    /// The bytes of the span; empty past the end.
    pub bytes: Vec<u8>,
}

impl Client {
    pub(super) fn offered(&self, capability: Capability) -> Result<(), Error> {
        if self.agreement.caps.contains(&capability) {
            Ok(())
        } else {
            Err(Error::NotOffered(capability))
        }
    }

    pub(super) fn call(&self, body: Request, payload: Option<&[u8]>) -> Result<Answer, Error> {
        self.call_within(body, payload, None)
    }

    /// Like `call`, but gives up waiting for the answer after `within` ([`Error::NoAnswer`]).
    /// `None` waits for as long as the link lives.
    pub(super) fn call_within(
        &self,
        body: Request,
        payload: Option<&[u8]>,
        within: Option<Duration>,
    ) -> Result<Answer, Error> {
        let sent = self.send_call(body, payload)?;
        self.answer_of(sent, within)
    }

    /// Sends a request without waiting for its answer, so several can be on the line at once;
    /// [`Client::answer_of`] waits for it.
    pub(super) fn send_call(
        &self,
        body: Request,
        payload: Option<&[u8]>,
    ) -> Result<InFlight, Error> {
        let id = RequestId(self.next.fetch_add(1, Ordering::Relaxed) + 1);
        let waiting = self.pending.register(id)?;
        let message = temor_wire::message::Message::Request { id, body };
        let sent = self
            .writer
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .send(&message, payload);
        if let Err(error) = sent {
            self.pending.forget(id);
            return Err(error);
        }
        Ok(InFlight { id, waiting })
    }

    /// The answer to a request [`Client::send_call`] sent. `None` waits for as long as the link
    /// lives.
    pub(super) fn answer_of(
        &self,
        InFlight { id, waiting }: InFlight,
        within: Option<Duration>,
    ) -> Result<Answer, Error> {
        let answer = match within {
            None => waiting.recv().map_err(|_| Error::LinkClosed),
            Some(within) => waiting.recv_timeout(within).map_err(|error| match error {
                RecvTimeoutError::Timeout => Error::NoAnswer,
                RecvTimeoutError::Disconnected => Error::LinkClosed,
            }),
        };
        if matches!(answer, Err(Error::NoAnswer)) {
            self.pending.forget(id);
        }
        answer?
    }

    pub(super) fn ask(&self, capability: Capability, body: Request) -> Result<Reply, Error> {
        self.offered(capability)?;
        self.call(body, None)?.outcome.map_err(Error::Remote)
    }

    /// The sessions the host holds for this person.
    pub fn list_sessions(&self) -> Result<Vec<SessionInfo>, Error> {
        match self.ask(Capability::Sessions, Request::ListSessions)? {
            Reply::Sessions(sessions) => Ok(sessions),
            _ => Err(Error::UnexpectedReply),
        }
    }

    /// Starts a session. It begins detached: [`Client::attach`] it to see its screen.
    pub fn open_session(&self, request: OpenSession) -> Result<SessionId, Error> {
        match self.ask(Capability::Sessions, Request::OpenSession(request))? {
            Reply::Opened { session } => Ok(session),
            _ => Err(Error::UnexpectedReply),
        }
    }

    /// Attaches to a session: its screen as a snapshot, and from then on its output as events.
    /// Attaching a session another link holds takes it over.
    pub fn attach(&self, session: SessionId) -> Result<Attached, Error> {
        self.offered(Capability::Sessions)?;
        let answer = self.call(Request::AttachSession { session }, None)?;
        match (answer.outcome.map_err(Error::Remote)?, answer.payload) {
            (Reply::Attached { session, size }, Some(snapshot)) => Ok(Attached {
                session,
                size,
                snapshot,
            }),
            _ => Err(Error::UnexpectedReply),
        }
    }

    /// Detaches: the session keeps running with its screen.
    pub fn detach(&self, session: SessionId) -> Result<(), Error> {
        self.ack(Capability::Sessions, Request::DetachSession { session })
    }

    /// Ends a session: its program is hung up and nothing of it survives.
    pub fn close_session(&self, session: SessionId) -> Result<(), Error> {
        self.ack(Capability::Sessions, Request::Close { session })
    }

    /// Resizes a session's screen and terminal. `cell` is one cell in pixels, for the program's
    /// pixel size reports.
    pub fn resize(&self, session: SessionId, size: Size, cell: CellPx) -> Result<(), Error> {
        self.ack(
            Capability::Sessions,
            Request::ResizeSession {
                session,
                size,
                cell,
            },
        )
    }

    /// Sends keys, a paste or a mouse report to a session's program.
    pub fn send_input(&self, session: SessionId, bytes: &[u8]) -> Result<(), Error> {
        self.offered(Capability::Sessions)?;
        let answer = self.call(Request::SendInput { session }, Some(bytes))?;
        answer.outcome.map(drop).map_err(Error::Remote)
    }

    /// Sets how long detached sessions are kept.
    pub fn set_idle_limit(&self, limit: IdleLimit) -> Result<(), Error> {
        self.ack(Capability::Sessions, Request::SetIdleLimit { limit })
    }

    /// The listening ports of this person's sessions now.
    pub fn list_ports(&self) -> Result<Vec<ListeningPort>, Error> {
        match self.ask(Capability::Ports, Request::ListPorts)? {
            Reply::Ports(ports) => Ok(ports),
            _ => Err(Error::UnexpectedReply),
        }
    }

    /// The entries of a remote folder, by name.
    pub fn list_dir(&self, path: &RemotePath) -> Result<Vec<DirEntry>, Error> {
        match self.ask(Capability::Files, Request::ListDir { path: path.clone() })? {
            Reply::Listing(entries) => Ok(entries),
            _ => Err(Error::UnexpectedReply),
        }
    }

    /// A remote path's metadata.
    pub fn stat(&self, path: &RemotePath) -> Result<FileMeta, Error> {
        match self.ask(Capability::Files, Request::Stat { path: path.clone() })? {
            Reply::Stat(meta) => Ok(meta),
            _ => Err(Error::UnexpectedReply),
        }
    }

    /// A span of a remote file.
    pub fn read_range(&self, path: &RemotePath, range: ByteRange) -> Result<ReadChunk, Error> {
        let sent = self.send_read(path, range)?;
        self.read_answer(sent)
    }

    /// Asks for a span of a remote file without waiting; [`Client::read_answer`] reads it.
    pub(super) fn send_read(&self, path: &RemotePath, range: ByteRange) -> Result<InFlight, Error> {
        self.offered(Capability::Files)?;
        let request = Request::ReadFile {
            path: path.clone(),
            range,
        };
        self.send_call(request, None)
    }

    /// The span a [`Client::send_read`] asked for.
    pub(super) fn read_answer(&self, sent: InFlight) -> Result<ReadChunk, Error> {
        let answer = self.answer_of(sent, None)?;
        match (answer.outcome.map_err(Error::Remote)?, answer.payload) {
            (Reply::Read { stamp, total }, Some(bytes)) => Ok(ReadChunk {
                stamp,
                total,
                bytes,
            }),
            _ => Err(Error::UnexpectedReply),
        }
    }

    /// Writes a remote file after `check`; the file's new stamp. A file that is not what the
    /// check expected is [`Error::Remote`] holding a conflict.
    pub fn write_file(
        &self,
        path: &RemotePath,
        check: WriteCheck,
        bytes: &[u8],
    ) -> Result<FileStamp, Error> {
        self.offered(Capability::Files)?;
        let request = Request::WriteFile {
            path: path.clone(),
            check,
            mode: WriteMode::Whole,
        };
        match self
            .call(request, Some(bytes))?
            .outcome
            .map_err(Error::Remote)?
        {
            Reply::Written(stamp) => Ok(stamp),
            _ => Err(Error::UnexpectedReply),
        }
    }

    /// Watches a remote path; changes arrive as
    /// [`ClientEvent::FileChanged`](super::ClientEvent::FileChanged).
    pub fn watch(&self, path: &RemotePath) -> Result<WatchId, Error> {
        match self.ask(Capability::Files, Request::Watch { path: path.clone() })? {
            Reply::Watching { watch } => Ok(watch),
            _ => Err(Error::UnexpectedReply),
        }
    }

    /// Stops watching.
    pub fn unwatch(&self, watch: WatchId) -> Result<(), Error> {
        self.ack(Capability::Files, Request::Unwatch { watch })
    }

    pub(super) fn ack(&self, capability: Capability, body: Request) -> Result<(), Error> {
        match self.ask(capability, body)? {
            Reply::Ack => Ok(()),
            _ => Err(Error::UnexpectedReply),
        }
    }
}
