//! The daemon's local socket (`$XDG_RUNTIME_DIR/temor-remote/<hash>.sock`, mode 0600) inside a
//! folder only the person can enter.

use std::fs;
use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;

use crate::error::Error;

/// Makes `path` (and its parents) if they are missing, readable by the person only.
pub fn ensure_private_dir(path: &Path) -> Result<(), Error> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
        .map_err(|error| Error::io(format!("making {}", path.display()), error))
}

/// One end of a connection. `Send`; clone it for the other direction with [`Half::duplicate`].
#[derive(Debug)]
pub struct Half(UnixStream);

impl Half {
    /// Another handle on the same connection, to read on one thread and write on another.
    pub fn duplicate(&self) -> Result<Self, Error> {
        self.0
            .try_clone()
            .map(Self)
            .map_err(|error| Error::io("sharing the socket", error))
    }

    /// Tells the other end no more bytes will be written.
    pub fn shutdown_write(&self) {
        // Already closed is the same as done.
        let _ = self.0.shutdown(Shutdown::Write);
    }
}

impl Read for Half {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl Write for Half {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

/// The daemon's listening socket.
#[derive(Debug)]
pub struct Listener(UnixListener);

impl Listener {
    /// Listens at `path`, which must not exist, with mode 0600.
    pub fn bind(path: &Path) -> Result<Self, Error> {
        let listener = UnixListener::bind(path)
            .map_err(|error| Error::io(format!("listening at {}", path.display()), error))?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|error| Error::io("restricting the socket", error))?;
        Ok(Self(listener))
    }

    /// Waits for the next connection.
    pub fn accept(&self) -> Result<Half, Error> {
        self.0
            .accept()
            .map(|(stream, _)| Half(stream))
            .map_err(|error| Error::io("accepting a connection", error))
    }
}

/// Connects to the daemon's socket.
pub fn connect(path: &Path) -> Result<Half, Error> {
    UnixStream::connect(path)
        .map(Half)
        .map_err(|error| Error::io(format!("connecting to {}", path.display()), error))
}
