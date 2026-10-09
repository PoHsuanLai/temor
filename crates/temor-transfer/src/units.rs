//! Byte counts, entry counts and the caps a caller injects.

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

/// A count of files, folders or open files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Count(u64);

impl Count {
    pub const fn new(count: u64) -> Self {
        Self(count)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for Count {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} entries", self.0)
    }
}

/// Extra room, in raw bytes, for the keys and names around a payload's data.
const HEADER_ALLOWANCE: u64 = 8192;

/// The caps for one transfer. The caller picks them all; nothing here has a default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    max_bytes: Bytes,
    max_entries: Count,
    max_open: Count,
}

impl Limits {
    /// `max_bytes` caps decoded data for one file or one transfer. `max_entries` caps files and
    /// folders in one transfer. `max_open` caps files being written at the same time.
    pub const fn new(max_bytes: Bytes, max_entries: Count, max_open: Count) -> Self {
        Self {
            max_bytes,
            max_entries,
            max_open,
        }
    }

    /// The most decoded bytes one file (or one whole transfer) may carry.
    pub const fn max_bytes(self) -> Bytes {
        self.max_bytes
    }

    /// The most files and folders one transfer may create.
    pub const fn max_entries(self) -> Count {
        self.max_entries
    }

    /// The most files that may be open for writing at once.
    pub const fn max_open(self) -> Count {
        self.max_open
    }

    /// The longest raw payload a parser accepts: the base64 that carries `max_bytes`, plus room
    /// for keys and names. Longer input is refused before anything is decoded.
    pub fn max_raw(self) -> Bytes {
        let encoded = self.max_bytes.get().div_ceil(3).saturating_mul(4);
        Bytes::new(encoded.saturating_add(HEADER_ALLOWANCE))
    }
}
