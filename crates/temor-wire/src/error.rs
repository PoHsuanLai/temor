//! The crate's one error type.

/// What went wrong with the bytes or the framing, shaped so the caller can act on it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum WireError {
    /// A frame announces more bytes than the limit allows, so it is refused before its body is
    /// buffered.
    #[error("a frame of {len} bytes is over the {limit}-byte limit")]
    Oversize {
        /// The bytes the frame announced (JSON and payload together).
        len: u64,
        /// The limit in force.
        limit: u64,
    },
    /// The stream ended in the middle of a frame.
    #[error("the stream ended in the middle of a frame")]
    Truncated,
    /// A frame's JSON is not a message of the expected type.
    #[error("a frame is not a valid message: {reason}")]
    Malformed {
        /// The parser's reason, for a log.
        reason: String,
    },
    /// A message could not be written as a frame.
    #[error("a message could not be encoded: {reason}")]
    Unencodable {
        /// The encoder's reason, for a log.
        reason: String,
    },
    /// An earlier frame was refused, so the decoder accepts no more bytes. The stream is over.
    #[error("the decoder refused an earlier frame and accepts no more bytes")]
    Spent,
}
