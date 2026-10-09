//! Reading a frame from a peer, which may be older or newer than this build.
//!
//! The reader classifies by tag, never by error text:
//! - A top-level kind this build does not know is `Ignored`.
//! - A request whose kind is unknown is `UnknownRequest`; one whose kind is known but whose body
//!   is not readable is `UnreadableRequest`. Both are answered under the request's id.
//! - A response whose outcome, reply or error is not readable fails its request by id.
//! - An event whose kind is unknown is `Ignored`; one whose kind is known but whose body is not
//!   readable is `UnreadableEvent`, so the caller can surface it (an exit is never lost silently).
//!
//! A well-framed message from a newer peer never ends the stream. Only a frame that is not JSON, a
//! greeting that cannot be read, or a message whose payload contradicts its kind is malformed.

use super::kinds::{EVENT_KINDS, MESSAGE_KINDS, REMOTE_ERROR_KINDS, REPLY_KINDS, REQUEST_KINDS};
use super::{Event, Message, RemoteError, Reply, Request};
use crate::WireError;
use crate::ids::RequestId;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// A message from a peer, with its payload when it carries one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// The message.
    pub message: Message,
    /// The payload: `Some` exactly when [`Message::carries_payload`] is true.
    pub payload: Option<Vec<u8>>,
}

/// What one frame turned out to be.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Incoming {
    /// A message this build reads in full.
    Frame(Frame),
    /// A request of a kind this build does not know. Answer with [`Message::unknown_request`].
    UnknownRequest {
        /// The request's id.
        id: RequestId,
    },
    /// A request of a known kind whose body this build cannot read. Answer with
    /// [`Message::unreadable_request`]; the request was not run.
    UnreadableRequest {
        /// The request's id.
        id: RequestId,
    },
    /// A response this build cannot read: its outcome, reply or error is of a kind it does not
    /// know. The request it answers has failed, so the caller should fail that request.
    UnknownResponse {
        /// The request's id.
        id: RequestId,
    },
    /// An event of a known kind whose body this build cannot read. The caller should surface it.
    UnreadableEvent {
        /// The event's kind, for example `Exited`.
        kind: String,
    },
    /// A message or event of a kind this build does not know. Skip it.
    Ignored,
}

impl Incoming {
    /// Reads one frame's JSON and payload.
    pub(crate) fn parse(json: &[u8], payload: Option<&[u8]>) -> Result<Self, WireError> {
        let value: Value = serde_json::from_slice(json).map_err(malformed)?;
        let (kind, body) =
            entry(&value).ok_or_else(|| malformed_reason("a frame must name one message kind"))?;
        if !MESSAGE_KINDS.contains(&kind) {
            return Ok(Incoming::Ignored);
        }
        match kind {
            "Hello" => carried(Message::Hello(decode(body)?), payload),
            "Request" => request(body, payload),
            "Response" => response(body, payload),
            _ => event(body, payload),
        }
    }
}

/// The tag of a value written by serde, with the value it names: a unit variant is a string, any
/// other variant is an object with exactly one key.
fn entry(value: &Value) -> Option<(&str, &Value)> {
    match value {
        Value::String(name) => Some((name.as_str(), value)),
        Value::Object(entries) => {
            let mut entries = entries.iter();
            match (entries.next(), entries.next()) {
                (Some((name, inner)), None) => Some((name.as_str(), inner)),
                _ => None,
            }
        }
        _ => None,
    }
}

/// The request or response id, which every answer must carry.
fn id_of(body: &Value) -> Result<RequestId, WireError> {
    body.get("id")
        .and_then(Value::as_u64)
        .map(RequestId)
        .ok_or_else(|| malformed_reason("a request or response has no id"))
}

fn request(body: &Value, payload: Option<&[u8]>) -> Result<Incoming, WireError> {
    let id = id_of(body)?;
    let inner = body
        .get("body")
        .ok_or_else(|| malformed_reason("a request has no body"))?;
    let (kind, _) = entry(inner).ok_or_else(|| malformed_reason("a request names no kind"))?;
    if !REQUEST_KINDS.contains(&kind) {
        return Ok(Incoming::UnknownRequest { id });
    }
    match Request::deserialize(inner) {
        Ok(body) => carried(Message::Request { id, body }, payload),
        Err(_) => Ok(Incoming::UnreadableRequest { id }),
    }
}

fn response(body: &Value, payload: Option<&[u8]>) -> Result<Incoming, WireError> {
    let id = id_of(body)?;
    let outcome = body
        .get("outcome")
        .ok_or_else(|| malformed_reason("a response has no outcome"))?;
    let Some((tag, inner)) = entry(outcome) else {
        return Err(malformed_reason("an outcome must be Ok or Err"));
    };
    let unreadable = Incoming::UnknownResponse { id };
    match tag {
        "Ok" => {
            let Some((kind, _)) = entry(inner) else {
                return Err(malformed_reason("a reply names no kind"));
            };
            if !REPLY_KINDS.contains(&kind) {
                return Ok(unreadable);
            }
            match Reply::deserialize(inner) {
                Ok(reply) => carried(
                    Message::Response {
                        id,
                        outcome: Ok(reply),
                    },
                    payload,
                ),
                Err(_) => Ok(unreadable),
            }
        }
        "Err" => {
            let Some((kind, _)) = entry(inner) else {
                return Err(malformed_reason("an error names no kind"));
            };
            if !REMOTE_ERROR_KINDS.contains(&kind) {
                return Ok(unreadable);
            }
            match RemoteError::deserialize(inner) {
                Ok(error) => carried(
                    Message::Response {
                        id,
                        outcome: Err(error),
                    },
                    payload,
                ),
                Err(_) => Ok(unreadable),
            }
        }
        // Unknown outcome: the id is still known, so the request fails rather than hangs.
        _ => Ok(unreadable),
    }
}

fn event(body: &Value, payload: Option<&[u8]>) -> Result<Incoming, WireError> {
    let (kind, _) = entry(body).ok_or_else(|| malformed_reason("an event names no kind"))?;
    if !EVENT_KINDS.contains(&kind) {
        return Ok(Incoming::Ignored);
    }
    match Event::deserialize(body) {
        Ok(event) => carried(Message::Event(event), payload),
        Err(_) => Ok(Incoming::UnreadableEvent {
            kind: kind.to_string(),
        }),
    }
}

/// A message read whole. The payload must be present exactly when the message carries one.
fn carried(message: Message, payload: Option<&[u8]>) -> Result<Incoming, WireError> {
    if message.carries_payload() != payload.is_some() {
        return Err(malformed_reason("the payload does not match the message"));
    }
    Ok(Incoming::Frame(Frame {
        message,
        payload: payload.map(<[u8]>::to_vec),
    }))
}

fn decode<T: DeserializeOwned>(value: &Value) -> Result<T, WireError> {
    T::deserialize(value).map_err(malformed)
}

fn malformed(error: serde_json::Error) -> WireError {
    WireError::Malformed {
        reason: error.to_string(),
    }
}

fn malformed_reason(reason: &str) -> WireError {
    WireError::Malformed {
        reason: reason.to_string(),
    }
}
