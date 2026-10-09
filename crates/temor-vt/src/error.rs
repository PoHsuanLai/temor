//! The crate's one error type.

/// What went wrong in a terminal call, in terms a caller can act on.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum VtError {
    /// libghostty could not allocate memory.
    #[error("the terminal ran out of memory")]
    OutOfMemory,
    /// A value was outside what libghostty accepts (a size of zero, an unknown mode, a point
    /// outside the grid).
    #[error("the terminal rejected a value: {0}")]
    InvalidValue(&'static str),
    /// A snapshot could not be read back (truncated, corrupt or from another format version).
    #[error("the snapshot could not be restored")]
    SnapshotRejected,
    /// libghostty returned a result code this crate does not know.
    #[error("the terminal failed with code {0}")]
    Failed(i32),
}
