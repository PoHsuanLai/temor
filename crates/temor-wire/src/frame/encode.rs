//! Writing a message and its payload as one frame.

use super::HEADER;
use super::limit::FrameLimit;
use crate::WireError;
use serde::Serialize;

/// The bytes of one frame: `message` as JSON, then `payload` raw. Refuses anything over `limit`.
pub fn encode<T: Serialize>(
    message: &T,
    payload: &[u8],
    limit: FrameLimit,
) -> Result<Vec<u8>, WireError> {
    let json = serde_json::to_vec(message).map_err(|error| WireError::Unencodable {
        reason: error.to_string(),
    })?;
    let json_len = header_word(json.len(), limit)?;
    let payload_len = header_word(payload.len(), limit)?;
    limit.check(json_len, payload_len)?;
    let mut bytes = Vec::with_capacity(HEADER + json.len() + payload.len());
    bytes.extend_from_slice(&json_len.to_le_bytes());
    bytes.extend_from_slice(&payload_len.to_le_bytes());
    bytes.extend_from_slice(&json);
    bytes.extend_from_slice(payload);
    Ok(bytes)
}

/// A length as a header word. A part too long for a u32 is over any limit.
fn header_word(len: usize, limit: FrameLimit) -> Result<u32, WireError> {
    u32::try_from(len).map_err(|_| WireError::Oversize {
        len: len as u64,
        limit: u64::from(limit.0),
    })
}
