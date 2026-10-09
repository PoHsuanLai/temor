//! Byte counts and the caps a caller injects.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A count of bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Bytes(u64);

impl Bytes {
    pub const fn new(count: u64) -> Self {
        Self(count)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for Bytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} bytes", self.0)
    }
}

/// The caps for one transfer. The caller picks them; nothing here has a default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    max_bytes: Bytes,
}

impl Limits {
    pub const fn new(max_bytes: Bytes) -> Self {
        Self { max_bytes }
    }

    /// The most decoded bytes one file (or one whole transfer) may carry.
    pub const fn max_bytes(self) -> Bytes {
        self.max_bytes
    }
}
