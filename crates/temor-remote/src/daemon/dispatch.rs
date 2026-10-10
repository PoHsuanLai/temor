//! Requests to the daemon, one arm each. Capabilities are checked here: a request for something
//! the handshake did not agree is an `Unsupported` error, never a silent success.

use std::sync::Arc;

use temor_wire::hello::Capability;
use temor_wire::ids::{RequestId, SessionId};
use temor_wire::message::{RemoteError, Reply, Request};

use super::core::Daemon;
use super::link::Link;
use super::session::Session;
use crate::error::Error;

/// How a request was answered.
pub(super) enum Done {
    /// With this reply and no payload.
    Reply(Reply),
    /// With this reply and its payload.
    Payload(Reply, Vec<u8>),
    /// Already sent (an attach answers under the session's lock).
    Sent,
}

fn need(link: &Link, capability: Capability) -> Result<(), RemoteError> {
    if link.has(capability) {
        Ok(())
    } else {
        Err(RemoteError::Unsupported { capability })
    }
}

fn wire(error: Error) -> RemoteError {
    error.to_wire()
}

impl Daemon {
    fn session(&self, session: SessionId) -> Result<Arc<Session>, RemoteError> {
        self.registry
            .get(session)
            .ok_or(RemoteError::NoSuchSession { session })
    }

    /// Carries out one request for `link`.
    pub(super) fn handle(
        self: &Arc<Self>,
        link: &Arc<Link>,
        id: RequestId,
        body: Request,
        payload: Option<&[u8]>,
    ) -> Result<Done, RemoteError> {
        match body {
            Request::OpenSession(request) => {
                need(link, Capability::Sessions)?;
                let session = self.open_session(&request).map_err(wire)?;
                Ok(Done::Reply(Reply::Opened { session }))
            }
            Request::AttachSession { session } => {
                need(link, Capability::Sessions)?;
                self.session(session)?.attach(link, id).map_err(wire)?;
                Ok(Done::Sent)
            }
            Request::DetachSession { session } => {
                need(link, Capability::Sessions)?;
                self.session(session)?.detach(Some(link.id));
                Ok(Done::Reply(Reply::Ack))
            }
            Request::Close { session } => {
                need(link, Capability::Sessions)?;
                self.session(session)?.close();
                Ok(Done::Reply(Reply::Ack))
            }
            Request::ResizeSession { session, size } => {
                need(link, Capability::Sessions)?;
                self.session(session)?.resize(size).map_err(wire)?;
                Ok(Done::Reply(Reply::Ack))
            }
            Request::SendInput { session } => {
                need(link, Capability::Sessions)?;
                self.session(session)?
                    .send_input(payload.unwrap_or_default());
                Ok(Done::Reply(Reply::Ack))
            }
            Request::ListSessions => {
                need(link, Capability::Sessions)?;
                let sessions = self.registry.all().iter().map(|s| s.info()).collect();
                Ok(Done::Reply(Reply::Sessions(sessions)))
            }
            Request::SetIdleLimit { limit } => {
                need(link, Capability::Sessions)?;
                *self
                    .idle
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = limit;
                Ok(Done::Reply(Reply::Ack))
            }
            Request::ListPorts => {
                need(link, Capability::Ports)?;
                Ok(Done::Reply(Reply::Ports(self.scan_ports().map_err(wire)?)))
            }
            other => self.handle_files(link, other, payload),
        }
    }

    fn handle_files(
        &self,
        link: &Arc<Link>,
        body: Request,
        payload: Option<&[u8]>,
    ) -> Result<Done, RemoteError> {
        match body {
            Request::ListDir { path } => {
                need(link, Capability::Files)?;
                let entries = self.files.list_dir(&path).map_err(wire)?;
                Ok(Done::Reply(Reply::Listing(entries)))
            }
            Request::Stat { path } => {
                need(link, Capability::Files)?;
                Ok(Done::Reply(Reply::Stat(
                    self.files.stat(&path).map_err(wire)?,
                )))
            }
            Request::ReadFile { path, range } => {
                need(link, Capability::Files)?;
                let (stamp, total, bytes) = self.files.read(&path, range).map_err(wire)?;
                Ok(Done::Payload(Reply::Read { stamp, total }, bytes))
            }
            Request::WriteFile { path, check } => {
                need(link, Capability::Files)?;
                let bytes = payload.unwrap_or_default();
                let stamp = self.files.write(&path, &check, bytes).map_err(wire)?;
                Ok(Done::Reply(Reply::Written(stamp)))
            }
            Request::Watch { path } => {
                need(link, Capability::Files)?;
                let seen = self.files.observe(&path).map_err(wire)?;
                let watch = link
                    .watches
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .add(path, seen);
                Ok(Done::Reply(Reply::Watching { watch }))
            }
            Request::Unwatch { watch } => {
                need(link, Capability::Files)?;
                link.watches
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .remove(watch)
                    .ok_or(RemoteError::NoSuchWatch { watch })?;
                Ok(Done::Reply(Reply::Ack))
            }
            // Trial runs and the agent relay need the sandbox runner, which is not linked yet.
            Request::RunTrial(_) => Err(RemoteError::Unsupported {
                capability: Capability::Trial,
            }),
            Request::Relay { .. } => Err(RemoteError::Unsupported {
                capability: Capability::Relay,
            }),
            _ => Err(RemoteError::UnknownRequest),
        }
    }
}
