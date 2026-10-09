//! The crate's one error type.

use std::io;
use std::path::PathBuf;

/// What went wrong writing shell integration.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ShellError {
    /// The shell integration files could not be written into the runtime directory.
    #[error("could not write shell integration files to {dir}: {source}")]
    Shims {
        dir: PathBuf,
        #[source]
        source: io::Error,
    },
}
