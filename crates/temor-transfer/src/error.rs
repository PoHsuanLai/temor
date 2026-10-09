//! The crate's one error type.

use std::io;
use std::path::PathBuf;

use crate::units::{Bytes, Count};

/// What went wrong, shaped so the caller can act on it.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum TransferError {
    /// The transfer is bigger than the caller's limit.
    #[error("the file is larger than the limit of {limit}")]
    TooLarge { limit: Bytes },
    /// The transfer creates more files and folders than the limit allows.
    #[error("the transfer has more than {limit}")]
    TooManyEntries { limit: Count },
    /// More files would be open for writing at once than the limit allows.
    #[error("more than {limit} files would be open at once")]
    TooManyOpen { limit: Count },
    /// This platform cannot do the operation safely, so it is refused rather than approximated.
    #[error("{feature} is not supported on this platform")]
    Unsupported { feature: &'static str },
    /// The sequence was cut short by the terminal's own limit, so it cannot be decoded.
    #[error("the file arrived cut short")]
    Cut,
    /// The bytes do not form a valid transfer.
    #[error("the transfer is malformed: {0}")]
    Malformed(Malformed),
    /// A file name or path would leave the destination folder.
    #[error("the path {path:?} is not allowed")]
    UnsafePath { path: String },
    /// A file in the destination could not be written, read or moved.
    #[error("could not write {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    /// The downloads index exists but cannot be read back.
    #[error("the downloads index at {path} is unreadable: {source}")]
    Index {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

/// The particular way a transfer's bytes were malformed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Malformed {
    #[error("the sequence is not a transfer this crate reads")]
    NotATransfer,
    #[error("the sequence has no payload separator")]
    NoSeparator,
    #[error("the value of {key} is not valid")]
    BadValue { key: &'static str },
    #[error("a required field {key} is missing")]
    MissingField { key: &'static str },
    #[error("the payload is not valid base64")]
    BadBase64,
    #[error("the file has {actual}, but its header says {declared}")]
    SizeMismatch { declared: Bytes, actual: Bytes },
    #[error("a file part arrived with no file started")]
    PartWithoutStart,
    #[error("the file end arrived with no file started")]
    EndWithoutStart,
    #[error("a new file started before the last one ended")]
    StartWhileOpen,
    #[error("a file arrived with a file still open")]
    WholeWhileOpen,
}

impl From<Malformed> for TransferError {
    fn from(value: Malformed) -> Self {
        Self::Malformed(value)
    }
}
