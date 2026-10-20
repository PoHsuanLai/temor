//! TODO(platform:windows): the controlling terminal (the console) and the opener (`start`).

use std::io;
use std::path::Path;
use std::time::Duration;

use temor_transfer::kitty::Link;

use crate::cli::error::CliError;
use crate::cli::opener::Opener;

/// What can go wrong with the terminal or the opener here: neither is written yet.
#[derive(Debug, thiserror::Error)]
pub enum SystemError {
    #[error("This isn't supported on this computer yet")]
    Unsupported,
}

/// Never built: opening the terminal refuses first.
#[derive(Debug)]
pub struct Tty;

impl Link for Tty {
    fn write(&mut self, _bytes: &[u8]) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }

    fn read(&mut self, _timeout: Duration) -> io::Result<Option<Vec<u8>>> {
        Err(io::ErrorKind::Unsupported.into())
    }
}

pub fn open_tty() -> Result<Tty, CliError> {
    Err(SystemError::Unsupported.into())
}

#[derive(Debug, Clone, Copy)]
pub struct SystemOpener;

impl Opener for SystemOpener {
    fn open(&self, _path: &Path) -> Result<(), CliError> {
        Err(SystemError::Unsupported.into())
    }
}
