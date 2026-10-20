//! Requests to the daemon, one arm each. Capabilities are checked here: a request for something
//! the handshake did not agree is an `Unsupported` error, never a silent success.

use std::sync::Arc;

use temor_wire::hello::Capability;
use temor_wire::ids::{RequestId, SessionId};
use temor_wire::message::{RemoteError, Reply, Request, WriteMode};

use super::core::Daemon;
use super::link::Link;
use super::session::Session;
use crate::error::Error;
use crate::sync::lock;

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
        if let Some(capability) = body.capability() {
            need(link, capability)?;
        }
        match body {
            Request::OpenSession(request) => {
                let session = self.open_session(&request).map_err(wire)?;
                Ok(Done::Reply(Reply::Opened { session }))
            }
            Request::AttachSession { session } => {
                self.session(session)?.attach(link, id).map_err(wire)?;
                Ok(Done::Sent)
            }
            Request::DetachSession { session } => {
                self.session(session)?.detach(Some(link.id));
                Ok(Done::Reply(Reply::Ack))
            }
            Request::Close { session } => {
                self.session(session)?.close();
                Ok(Done::Reply(Reply::Ack))
            }
            Request::ResizeSession {
                session,
                size,
                cell,
            } => {
                self.session(session)?.resize(size, cell).map_err(wire)?;
                Ok(Done::Reply(Reply::Ack))
            }
            Request::SetPalette { session, palette } => {
                self.session(session)?.set_palette(&palette).map_err(wire)?;
                Ok(Done::Reply(Reply::Ack))
            }
            // Answered here on the reader thread, ahead of any file request still running, so a
            // slow download never looks like a dead link.
            Request::Ping => Ok(Done::Reply(Reply::Pong)),
            Request::SendInput { session } => {
                self.session(session)?
                    .send_input(payload.unwrap_or_default());
                Ok(Done::Reply(Reply::Ack))
            }
            Request::ListSessions => {
                let sessions = self.registry.all().iter().map(|s| s.info()).collect();
                Ok(Done::Reply(Reply::Sessions(sessions)))
            }
            Request::SetIdleLimit { limit } => {
                *lock(&self.idle) = limit;
                Ok(Done::Reply(Reply::Ack))
            }
            Request::ListPorts => Ok(Done::Reply(Reply::Ports(self.scan_ports().map_err(wire)?))),
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
                let entries = self.files.list_dir(&path).map_err(wire)?;
                Ok(Done::Reply(Reply::Listing(entries)))
            }
            Request::Stat { path } => Ok(Done::Reply(Reply::Stat(
                self.files.stat(&path).map_err(wire)?,
            ))),
            Request::ReadFile { path, range } => {
                let (stamp, total, bytes) = self.files.read(&path, range).map_err(wire)?;
                Ok(Done::Payload(Reply::Read { stamp, total }, bytes))
            }
            Request::WriteFile { path, check, mode } => {
                let bytes = payload.unwrap_or_default();
                let staged = match mode {
                    WriteMode::Part { offset, end } => self
                        .files
                        .write_part(&path, &check, offset, end, bytes)
                        .map_err(wire)?,
                    WriteMode::Whole => Some(self.files.write(&path, &check, bytes).map_err(wire)?),
                };
                Ok(Done::Reply(match staged {
                    Some(stamp) => Reply::Written(stamp),
                    None => Reply::Ack,
                }))
            }
            Request::Watch { path } => {
                let seen = self.files.observe(&path).map_err(wire)?;
                let watch = lock(&link.watches).add(path, seen);
                Ok(Done::Reply(Reply::Watching { watch }))
            }
            Request::Unwatch { watch } => {
                lock(&link.watches)
                    .remove(watch)
                    .ok_or(RemoteError::NoSuchWatch { watch })?;
                Ok(Done::Reply(Reply::Ack))
            }
            Request::Facts { session } => {
                // A session's folder is the client's to ask about: it opened the session there.
                let cwd = self.session(session)?.info().cwd;
                let facts = crate::facts::read_folder(
                    std::path::Path::new(&cwd.0),
                    self.config.home.as_deref(),
                );
                Ok(Done::Reply(Reply::Facts(facts)))
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
