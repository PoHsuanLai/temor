//! TODO(platform:macos), TODO(platform:windows): the controlling terminal (`/dev/tty` and
//! termios on macOS, the console on Windows) and the opener (`open`, `start`).

use std::io;
use std::path::Path;
use std::time::Duration;

use temor_transfer::kitty::Link;

use crate::cli::error::CliError;
use crate::cli::opener::Opener;

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
    Err(CliError::Unsupported)
}

#[derive(Debug, Clone, Copy)]
pub struct SystemOpener;

impl Opener for SystemOpener {
    fn open(&self, _path: &Path) -> Result<(), CliError> {
        Err(CliError::Unsupported)
    }
}
