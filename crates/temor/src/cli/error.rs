//! The one error of the subcommands. Every message is for a person, not a programmer.

use std::io;

use temor_transfer::TransferError;

use super::args::UsageError;
use super::platform::SystemError;
use super::text::transfer_words;

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("{0}")]
    Usage(#[from] UsageError),
    #[error("{}", transfer_words(.0))]
    Transfer(#[from] TransferError),
    #[error("Couldn't find this folder: {0}")]
    Folder(#[source] io::Error),
    /// The terminal and the opener, as this system has them (`platform`).
    #[error(transparent)]
    System(#[from] SystemError),
}
