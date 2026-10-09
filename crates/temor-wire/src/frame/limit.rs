//! The most bytes one frame may carry.

use super::HEADER;
use crate::WireError;
use serde::{Deserialize, Serialize};

/// The most bytes one frame may carry, JSON and payload together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FrameLimit(pub u32);

impl FrameLimit {
    /// 64 MiB: a full terminal snapshot with scrollback, or a ranged file read, fits well inside.
    pub const DEFAULT: Self = Self(64 << 20);

    /// Checks a pair of lengths against the limit and returns them as sizes.
    pub(crate) fn check(self, json: u32, payload: u32) -> Result<(usize, usize), WireError> {
        let len = u64::from(json) + u64::from(payload);
        if len > u64::from(self.0) {
            return Err(WireError::Oversize {
                len,
                limit: u64::from(self.0),
            });
        }
        // A u32 length fits in usize on every target temor builds for (32-bit and 64-bit).
        Ok((json as usize, payload as usize))
    }
}

/// The two lengths a header announces, read from its eight bytes.
pub(crate) fn header_lengths(
    header: &[u8; HEADER],
    limit: FrameLimit,
) -> Result<(usize, usize), WireError> {
    let [a, b, c, d, e, f, g, h] = *header;
    let json = u32::from_le_bytes([a, b, c, d]);
    let payload = u32::from_le_bytes([e, f, g, h]);
    limit.check(json, payload)
}
