//! A session: one shell, or one program, in one place, from when it started until it ended. Its
//! id lasts across launches and across the daemon that may keep it running; a pane's id is only
//! a window's handle on it for one launch.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::SchemaError;

/// A session's lasting id: 64 bits the store draws at random, written as 16 lowercase hex digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct SessionId(u64);

impl SessionId {
    /// The id with these bits. The store draws them; nothing here chooses one.
    pub const fn from_bits(bits: u64) -> SessionId {
        SessionId(bits)
    }

    /// The bits, for a store that keeps the id as a number.
    pub const fn bits(self) -> u64 {
        self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

impl FromStr for SessionId {
    type Err = SchemaError;
    fn from_str(text: &str) -> Result<SessionId, SchemaError> {
        let hex = text.len() == 16 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        match hex {
            true => u64::from_str_radix(text, 16)
                .map(SessionId)
                .map_err(|_| SchemaError::BadSessionId(text.to_owned())),
            false => Err(SchemaError::BadSessionId(text.to_owned())),
        }
    }
}

impl From<SessionId> for String {
    fn from(id: SessionId) -> String {
        id.to_string()
    }
}

impl TryFrom<String> for SessionId {
    type Error = SchemaError;
    fn try_from(text: String) -> Result<SessionId, SchemaError> {
        text.parse()
    }
}

/// A moment on the wall clock, in milliseconds since the Unix epoch.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct At(pub i64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_is_sixteen_hex_digits_both_ways() {
        let id = SessionId::from_bits(0x7f3a_0000_0000_00ff);
        assert_eq!(id.to_string(), "7f3a0000000000ff");
        assert_eq!("7f3a0000000000ff".parse::<SessionId>(), Ok(id));
        for bad in [
            "",
            "7f3a",
            "7F3A0000000000FF",
            "7f3a0000000000ffa",
            "+f3a0000000000ff",
        ] {
            assert!(bad.parse::<SessionId>().is_err(), "{bad:?}");
        }
        let json = serde_json::to_string(&id).expect("json");
        assert_eq!(json, "\"7f3a0000000000ff\"");
        assert_eq!(serde_json::from_str::<SessionId>(&json).ok(), Some(id));
    }
}
