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
    let json_len = json_word(json.len(), limit)?;
    let payload_field = match payload {
        Some(bytes) => Some(payload_word(bytes.len(), limit)?),
        None => None,
    };
    let lengths = check(json_len, payload_field, limit)?;
    let mut bytes = Vec::with_capacity(HEADER + lengths.json + lengths.payload.unwrap_or(0));
    bytes.extend_from_slice(&json_len.to_le_bytes());
    bytes.extend_from_slice(&payload_field.unwrap_or(NO_PAYLOAD).to_le_bytes());
    bytes.extend_from_slice(&json);
    if let Some(bytes_of_payload) = payload {
        bytes.extend_from_slice(bytes_of_payload);
    }
    Ok(bytes)
}

/// The JSON length as a header word. Too long for a u32 is over any limit.
fn json_word(len: usize, limit: FrameLimit) -> Result<u32, WireError> {
    u32::try_from(len).map_err(|_| limit.refuse(len as u64))
}

/// The payload length as a header word. A payload of exactly `u32::MAX` bytes would read back as
/// "no payload", so it cannot be sent at all; that is an encoding error, not a size error.
pub(crate) fn payload_word(len: usize, limit: FrameLimit) -> Result<u32, WireError> {
    let word = u32::try_from(len).map_err(|_| limit.refuse(len as u64))?;
    if word == NO_PAYLOAD {
        return Err(WireError::Unencodable {
            reason: "a payload of exactly 4294967295 bytes collides with the no-payload marker"
                .to_string(),
        });
    }
    Ok(word)
}
