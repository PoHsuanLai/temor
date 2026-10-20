//! The crate's one error type.

use std::io;

use temor_wire::WireError;
use temor_wire::hello::{Capability, Incompatibility};
use temor_wire::message::{FileStamp, RemoteError, RemotePath};

/// What went wrong, shaped so the caller can act on it. `to_wire` turns the ones a request can
/// cause into the typed reply the other end reads.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The framing or the messages were wrong.
    #[error("the link broke: {0}")]
    Wire(#[from] WireError),
    /// The link ended; the write error that showed it, when there was one.
    #[error("the link ended")]
    LinkClosed(#[source] Option<io::Error>),
    /// The first message from the other end was not its greeting.
    #[error("the other end did not start with a greeting")]
    NoGreeting,
    /// The two ends share no protocol version.
    #[error("the host and this temor share no protocol version: {0:?}")]
    Incompatible(Incompatibility),
    /// The other end did not offer a capability that was asked for.
    #[error("the host does not offer {0:?}")]
    NotOffered(Capability),
    /// The host did not answer a ping in time.
    #[error("the host did not answer in time")]
    NoAnswer,
    /// The host answered a request with an error.
    #[error("the host refused: {0:?}")]
    Remote(RemoteError),
    /// The host answered with a reply that does not fit the request.
    #[error("the host answered with something else than was asked")]
    UnexpectedReply,
    /// The terminal (pseudo-terminal) side failed.
    #[error("the terminal failed: {0}")]
    Pty(#[from] temor_pty::PtyError),
    /// The screen state (libghostty) failed.
    #[cfg(feature = "ghostty")]
    #[error("the screen state failed: {0}")]
    Screen(#[from] temor_vt::VtError),
    /// A local file could not be written into the downloads folder.
    #[error("the download failed: {0}")]
    Transfer(#[from] temor_transfer::TransferError),
    /// A path is not one the client was given access to.
    #[error("{path} is outside what was opened")]
    Outside {
        /// The path.
        path: String,
    },
    /// A path is not absolute, has a `..`, or names the wrong kind of entry.
    #[error("{path} is not a usable path")]
    BadPath {
        /// The path.
        path: String,
    },
    /// The path does not exist.
    #[error("{path} was not found")]
    NotFound {
        /// The path.
        path: String,
    },
    /// The host's permissions refuse the path.
    #[error("{path} is not allowed by the host")]
    PermissionDenied {
        /// The path.
        path: String,
    },
    /// A write found the file changed (or present when it must be new).
    #[error("the file changed since it was read")]
    Conflict {
        /// The file's stamp now, or `None` if it is gone.
        current: Option<FileStamp>,
    },
    /// The file changed while it was being fetched.
    #[error("the file changed while it was being copied")]
    ChangedWhileCopying,
    /// A part of a streamed upload did not start where the staged bytes end.
    #[error("the parts of the upload arrived out of order")]
    PartOutOfOrder {
        /// How many bytes were staged.
        have: u64,
    },
    /// A terminal size with no columns or no rows.
    #[error("a terminal needs at least one column and one row")]
    BadSize,
    /// This host cannot do that.
    #[error("{feature} is not supported on this host")]
    Unsupported {
        /// What was asked.
        feature: &'static str,
    },
    /// An operating-system call failed.
    #[error("{context}: {source}")]
    Io {
        /// What was being done.
        context: String,
        /// The error.
        #[source]
        source: io::Error,
    },
}

impl Error {
    /// An I/O failure with what was being done.
    pub fn io(context: impl Into<String>, source: io::Error) -> Self {
        Self::Io {
            context: context.into(),
            source,
        }
    }

    /// Whether the link itself is gone or unreadable, rather than the request being refused.
    pub fn is_link_lost(&self) -> bool {
        matches!(
            self,
            Self::LinkClosed(_) | Self::Wire(_) | Self::NoAnswer | Self::NoGreeting
        )
    }

    /// Whether the file or folder asked for is not there, however the failure arrived: the
    /// helper's typed answer or this crate's own.
    pub fn is_not_found(&self) -> bool {
        matches!(
            self,
            Self::NotFound { .. } | Self::Remote(RemoteError::NotFound { .. })
        )
    }

    /// For a write the file's state refused: the file's stamp now (`None` if it is gone). `None`
    /// for any other failure.
    pub fn conflict(&self) -> Option<Option<FileStamp>> {
        match self {
            Self::Conflict { current } | Self::Remote(RemoteError::Conflict { current }) => {
                Some(*current)
            }
            _ => None,
        }
    }

    /// The typed error a response carries for this failure.
    pub fn to_wire(&self) -> RemoteError {
        match self {
            Self::NotFound { path } => RemoteError::NotFound {
                path: RemotePath(path.clone()),
            },
            Self::PermissionDenied { path } | Self::Outside { path } => {
                RemoteError::PermissionDenied {
                    path: RemotePath(path.clone()),
                }
            }
            Self::Conflict { current } => RemoteError::Conflict { current: *current },
            Self::BadPath { path } => RemoteError::BadRequest {
                detail: format!("not a usable path: {path}"),
            },
            Self::PartOutOfOrder { have } => RemoteError::BadRequest {
                detail: format!(
                    "the upload has {have} bytes staged and this part starts elsewhere"
                ),
            },
            Self::BadSize => RemoteError::BadRequest {
                detail: self.to_string(),
            },
            Self::NotOffered(capability) => RemoteError::Unsupported {
                capability: *capability,
            },
            Self::Remote(error) => error.clone(),
            other => RemoteError::Io {
                detail: other.to_string(),
            },
        }
    }
}
