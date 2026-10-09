//! The eight-byte header: two little-endian words, read and checked against the limit.

use super::limit::FrameLimit;
use crate::WireError;

/// Bytes in a frame header: two `u32` words.
pub(crate) const HEADER: usize = 8;

/// The payload word that means "this frame has no payload".
pub(crate) const NO_PAYLOAD: u32 = u32::MAX;

/// The lengths a header announces. `payload` is `None` for a frame without one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Lengths {
    pub(crate) json: usize,
    pub(crate) payload: Option<usize>,
}

/// Reads a header, refusing it when its JSON and payload together are over the limit. The check
/// happens before any body is waited for.
pub(crate) fn read(header: &[u8; HEADER], limit: FrameLimit) -> Result<Lengths, WireError> {
    let [a, b, c, d, e, f, g, h] = *header;
    let json = u32::from_le_bytes([a, b, c, d]);
    let payload = u32::from_le_bytes([e, f, g, h]);
    let payload = (payload != NO_PAYLOAD).then_some(payload);
    check(json, payload, limit)
}

/// Checks a pair of lengths against the limit and returns them as sizes.
pub(crate) fn check(
    json: u32,
    payload: Option<u32>,
    limit: FrameLimit,
) -> Result<Lengths, WireError> {
    let body = u64::from(json) + u64::from(payload.unwrap_or(0));
    if body > u64::from(limit.0) {
        return Err(WireError::Oversize {
            len: body,
            limit: u64::from(limit.0),
        });
    }
    // A u32 length fits in usize on every target temor builds for (32-bit and 64-bit).
    Ok(Lengths {
        json: json as usize,
        payload: payload.map(|len| len as usize),
    })
}

/// The whole frame's size in bytes, header included. Checked, so a pair of lengths can never wrap.
pub(crate) fn total(lengths: Lengths) -> Option<usize> {
    let body = lengths.json.checked_add(lengths.payload.unwrap_or(0))?;
    HEADER.checked_add(body)
}
