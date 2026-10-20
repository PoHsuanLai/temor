//! Content hashes: the helper's identity and a file's fingerprint.

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize, Serializer};

/// A 32-byte content hash (SHA-256 by convention), written as 64 lowercase hex digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContentHash(pub [u8; 32]);

impl ContentHash {
    /// The first 16 hex digits of the hash: enough to name a file or a socket after it.
    pub fn short(self) -> String {
        let mut hex = self.to_hex();
        hex.truncate(16);
        hex
    }

    /// The hash written as 64 lowercase hex digits.
    pub fn to_hex(self) -> String {
        use std::fmt::Write;
        let mut text = String::with_capacity(64);
        for byte in self.0 {
            // Writing to a String cannot fail.
            let _ = write!(text, "{byte:02x}");
        }
        text
    }

    /// Parses exactly 64 hex digits, in either case. Anything else is `None`.
    pub fn from_hex(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        if bytes.len() != 64 {
            return None;
        }
        let mut out = [0u8; 32];
        let (pairs, _) = bytes.as_chunks::<2>();
        for (slot, [high, low]) in out.iter_mut().zip(pairs) {
            *slot = (nibble(*high)? << 4) | nibble(*low)?;
        }
        Some(Self(out))
    }
}

fn nibble(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        b'A'..=b'F' => Some(digit - b'A' + 10),
        _ => None,
    }
}

impl Serialize for ContentHash {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for ContentHash {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::from_hex(&text).ok_or_else(|| de::Error::custom("expected 64 hex digits"))
    }
}
