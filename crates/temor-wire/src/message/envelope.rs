//! The one type every frame carries, and the rule for which of them carry a payload.

use super::{Event, RemoteError, Reply, Request};
use crate::hello::Hello;
use crate::ids::RequestId;
use serde::{Deserialize, Serialize};

/// A frame's message: the greeting, a request, its answer, or an event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Message {
    /// The greeting, sent first by each side.
    Hello(Hello),
    /// A request from temor to the helper.
    Request {
        /// Names the request, so its response can be matched.
        id: RequestId,
        /// What is asked.
        body: Request,
    },
    /// The answer to a request: its reply, or the typed reason it failed. Never both, and never
    /// neither: a request always gets exactly one response.
    Response {
        /// The request this answers.
        id: RequestId,
        /// The reply, or the error.
        outcome: Result<Reply, RemoteError>,
    },
    /// Something the helper reports without being asked.
    Event(Event),
}

impl Message {
    /// Whether this message's frame carries a payload: the bytes of input, of a write, of a
    /// snapshot, of a ranged read, or of terminal output. Every other message has none.
    pub fn carries_payload(&self) -> bool {
        matches!(
            self,
            Message::Request {
                body: Request::SendInput { .. } | Request::WriteFile { .. },
                ..
            } | Message::Response {
                outcome: Ok(Reply::Attached { .. } | Reply::Read { .. }),
                ..
            } | Message::Event(Event::Output { .. })
        )
    }

    /// The answer to a request whose kind this build does not know: a typed error under its id.
    pub fn unknown_request(id: RequestId) -> Self {
        Message::Response {
            id,
            outcome: Err(RemoteError::UnknownRequest),
        }
    }

    /// The answer to a request whose kind this build knows but whose body it cannot read in full:
    /// a typed error under its id, so the request is not left hanging.
    pub fn unreadable_request(id: RequestId) -> Self {
        Message::Response {
            id,
            outcome: Err(RemoteError::Unreadable {
                detail: "this build cannot read the request, so it was not run".to_string(),
            }),
        }
    }
}
