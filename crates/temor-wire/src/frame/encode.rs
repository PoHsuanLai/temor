//! Writing a message and its payload as one frame.

use super::header::{HEADER, NO_PAYLOAD, check};
use super::limit::FrameLimit;
use crate::WireError;
use crate::message::Message;

/// The bytes of one frame: `message` as JSON, then `payload` raw.
///
/// The payload must be present exactly when the message carries one (see
/// [`Message::carries_payload`]). Anything over `limit` is refused.
pub fn encode(
    message: &Message,
    payload: Option<&[u8]>,
    limit: FrameLimit,
) -> Result<Vec<u8>, WireError> {
    if message.carries_payload() != payload.is_some() {
        return Err(WireError::Unencodable {
            reason: "the payload does not match the message".to_string(),
        });
    }
    let json = serde_json::to_vec(message).map_err(|error| WireError::Unencodable {
        reason: error.to_string(),
    })?;
    let json_len = word(json.len(), limit)?;
    let payload_word = match payload {
        Some(bytes) => word(bytes.len(), limit)?,
        None => NO_PAYLOAD,
    };
    let lengths = check(json_len, payload.map(|_| payload_word), limit)?;
    let mut bytes = Vec::with_capacity(HEADER + lengths.json + lengths.payload.unwrap_or(0));
    bytes.extend_from_slice(&json_len.to_le_bytes());
    bytes.extend_from_slice(&payload_word.to_le_bytes());
    bytes.extend_from_slice(&json);
    if let Some(bytes_of_payload) = payload {
        bytes.extend_from_slice(bytes_of_payload);
    }
    Ok(bytes)
}

/// A length as a header word. A part too long for a u32 is over any limit.
fn word(len: usize, limit: FrameLimit) -> Result<u32, WireError> {
    u32::try_from(len)
        .ok()
        .filter(|word| *word != NO_PAYLOAD)
        .ok_or_else(|| limit.refuse(len as u64))
}
