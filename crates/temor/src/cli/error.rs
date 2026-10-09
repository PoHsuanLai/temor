//! The one error of the subcommands. Every message is for a person, not a programmer.

use std::io;
use std::path::PathBuf;

use temor_transfer::TransferError;

use super::args::UsageError;
use super::text::transfer_words;

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("{0}")]
    Usage(#[from] UsageError),
    #[error("This only works in a terminal window")]
    NoTerminal,
    #[error("Couldn't talk to the terminal: {0}")]
    Terminal(#[source] io::Error),
    #[error("{}", transfer_words(.0))]
    Transfer(#[from] TransferError),
    #[error("Couldn't find this folder: {0}")]
    Folder(#[source] io::Error),
    #[error("There is no program on this computer to open {}", .0.display())]
    NoOpener(PathBuf),
    #[error("Couldn't open {}: {}", .path.display(), .source)]
    Open {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("This isn't supported on this computer yet")]
    #[allow(dead_code)] // built only by the stubs for other systems
    Unsupported,
}
