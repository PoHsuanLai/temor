//! The most bytes one frame may carry.

use crate::WireError;
use serde::{Deserialize, Serialize};

/// The most bytes one frame may carry, JSON and payload together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FrameLimit(pub u32);

impl FrameLimit {
    /// 64 MiB: a full terminal snapshot with scrollback, or a ranged file read, fits well inside.
    pub const DEFAULT: Self = Self(64 << 20);

    /// The error for a frame whose body would be `len` bytes, when that is over the limit.
    pub(crate) fn refuse(self, len: u64) -> WireError {
        WireError::Oversize {
            len,
            limit: u64::from(self.0),
        }
    }
}
