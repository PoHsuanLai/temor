//! Reading a frame from a peer, which may be older or newer than this build.
//!
//! A newer peer can send a message kind this build does not know. Rather than end the stream, the
//! reader classifies it: an unknown request is answered with [`RemoteError::UnknownRequest`] under
//! its id, an unknown response fails the request it answers, and an unknown event is skipped. A
//! handshake that cannot be read, or a payload that does not match its message, is malformed.

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
pub enum Incoming {
    /// A message this build understands.
    Frame(Frame),
    /// A request this build does not know. Answer it with [`Message::unknown_request`].
    UnknownRequest {
        /// The request's id.
        id: RequestId,
    },
    /// A response whose reply this build does not know. The request it answers has failed to
    /// read, so the caller should fail that request.
    UnknownResponse {
        /// The request's id.
        id: RequestId,
    },
    /// An event, or some other message kind, this build does not know. Skip it.
    Ignored,
}

impl Incoming {
    /// Reads one frame's JSON and payload.
    pub(crate) fn parse(json: &[u8], payload: Option<&[u8]>) -> Result<Self, WireError> {
        let value: Value = serde_json::from_slice(json).map_err(malformed)?;
        let Value::Object(entries) = &value else {
            return Err(malformed_reason("a frame is not a JSON object"));
        };
        let mut entries = entries.iter();
        let (Some((kind, body)), None) = (entries.next(), entries.next()) else {
            return Err(malformed_reason(
                "a frame must name exactly one message kind",
            ));
        };
        match kind.as_str() {
            "Hello" => carried(Message::Hello(decode(body)?), payload),
            "Request" => request(body, payload),
            "Response" => response(body, payload),
            "Event" => event(body, payload),
            _ => Ok(Incoming::Ignored),
        }
    }
}

#[derive(Deserialize)]
struct RequestEnvelope {
    id: RequestId,
    body: Value,
}

#[derive(Deserialize)]
struct ResponseEnvelope {
    id: RequestId,
    outcome: Value,
}

fn request(body: &Value, payload: Option<&[u8]>) -> Result<Incoming, WireError> {
    let envelope: RequestEnvelope = decode(body)?;
    match Request::deserialize(&envelope.body) {
        Ok(body) => carried(
            Message::Request {
                id: envelope.id,
                body,
            },
            payload,
        ),
        Err(error) if is_unknown_variant(&error) => {
            Ok(Incoming::UnknownRequest { id: envelope.id })
        }
        Err(error) => Err(malformed(error)),
    }
}

fn response(body: &Value, payload: Option<&[u8]>) -> Result<Incoming, WireError> {
    let envelope: ResponseEnvelope = decode(body)?;
    let id = envelope.id;
    let Value::Object(entries) = &envelope.outcome else {
        return Err(malformed_reason("an outcome must be Ok or Err"));
    };
    let mut entries = entries.iter();
    let (Some((kind, inner)), None) = (entries.next(), entries.next()) else {
        return Err(malformed_reason("an outcome must be Ok or Err"));
    };
    let outcome = match kind.as_str() {
        "Ok" => match Reply::deserialize(inner) {
            Ok(reply) => Ok(reply),
            Err(error) if is_unknown_variant(&error) => {
                return Ok(Incoming::UnknownResponse { id });
            }
            Err(error) => return Err(malformed(error)),
        },
        "Err" => match RemoteError::deserialize(inner) {
            Ok(error) => Err(error),
            Err(error) if is_unknown_variant(&error) => {
                return Ok(Incoming::UnknownResponse { id });
            }
            Err(error) => return Err(malformed(error)),
        },
        _ => return Err(malformed_reason("an outcome must be Ok or Err")),
    };
    carried(Message::Response { id, outcome }, payload)
}

fn event(body: &Value, payload: Option<&[u8]>) -> Result<Incoming, WireError> {
    match Event::deserialize(body) {
        Ok(event) => carried(Message::Event(event), payload),
        Err(error) if is_unknown_variant(&error) => Ok(Incoming::Ignored),
        Err(error) => Err(malformed(error)),
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

/// serde reports a variant it does not know with this prefix, and no other error uses it.
fn is_unknown_variant(error: &serde_json::Error) -> bool {
    error.to_string().starts_with("unknown variant")
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
