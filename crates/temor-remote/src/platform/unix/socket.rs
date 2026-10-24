//! The daemon's local socket (`$XDG_RUNTIME_DIR/temor-remote/<hash>.sock`, mode 0600) inside a
//! folder only the person can enter.

use std::fs;
use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;

use temor_shell::ShellError;

use crate::error::Error;

/// Makes `path` (and its parents) if they are missing, readable by the person only. A folder
/// that is already there and is ours is tightened to 0700 (a mode like 0300 or 0000 would
/// otherwise fail the socket later); one that is not ours, or is a link, is refused. The check
/// is temor-shell's.
pub fn ensure_private_dir(path: &Path) -> Result<(), Error> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)
            .map_err(|error| Error::io(format!("making {}", parent.display()), error))?;
    }
    temor_shell::ensure_private_dir(path).map_err(|error| match error {
        ShellError::PrivateDir { source, .. } => {
            Error::io(format!("making {}", path.display()), source)
        }
        other => Error::io(
            format!("using {}", path.display()),
            io::Error::new(io::ErrorKind::PermissionDenied, other.to_string()),
        ),
    })
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

/// Which file a socket is (device and inode), so a daemon removes only the socket it made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SocketId(u64, u64);

fn socket_id(path: &Path) -> Option<SocketId> {
    fs::symlink_metadata(path)
        .ok()
        .map(|meta| SocketId(meta.dev(), meta.ino()))
}

/// Removes the socket at `path` if it is still the one `id` names: a daemon that started after
/// this one and bound the same path keeps its socket.
pub fn remove_socket(path: &Path, id: SocketId) {
    if socket_id(path) == Some(id) {
        let _ = fs::remove_file(path);
    }
}

/// An exclusive hold on the lock file beside a daemon's socket (an `flock`), for as long as the
/// value lives; the system lets go of it when the process ends, however it ends.
#[derive(Debug)]
pub struct SocketLock {
    _held: fs::File,
}

impl SocketLock {
    /// Takes the lock at `path`, made 0600 if it is missing (in the private folder, so nobody
    /// else can put a file there). `None` when another process holds it (a daemon is starting or
    /// running there).
    pub fn try_take(path: &Path) -> Result<Option<Self>, Error> {
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(path)
            .map_err(|error| Error::io(format!("opening {}", path.display()), error))?;
        match file.try_lock() {
            Ok(()) => Ok(Some(Self { _held: file })),
            Err(fs::TryLockError::WouldBlock) => Ok(None),
            Err(fs::TryLockError::Error(error)) => {
                Err(Error::io(format!("locking {}", path.display()), error))
            }
        }
    }
}

/// The daemon's listening socket.
#[derive(Debug)]
pub struct Listener(UnixListener, Option<SocketId>);

impl Listener {
    /// Listens at `path`, which must not exist, with mode 0600.
    pub fn bind(path: &Path) -> Result<Self, Error> {
        // A longer path is refused by the system with a vague error; say what is wrong.
        if path.as_os_str().len() > temor_shell::SOCKET_PATH_MAX {
            return Err(Error::io(
                format!("listening at {}", path.display()),
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "the socket's path is longer than a unix socket allows",
                ),
            ));
        }
        let listener = UnixListener::bind(path)
            .map_err(|error| Error::io(format!("listening at {}", path.display()), error))?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|error| Error::io("restricting the socket", error))?;
        Ok(Self(listener, socket_id(path)))
    }

    /// Which file the socket was when it was bound; `None` if it could not be told.
    pub fn id(&self) -> Option<SocketId> {
        self.1
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_process_holds_the_lock_and_a_socket_is_removed_only_while_it_is_ours() {
        let base = std::env::temp_dir().join(format!("temor-remote-lock-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        ensure_private_dir(&base).expect("made");
        let lock_at = base.join("local.lock");
        let held = SocketLock::try_take(&lock_at)
            .expect("opens")
            .expect("free");
        assert!(
            SocketLock::try_take(&lock_at).expect("opens").is_none(),
            "a second taker waits for the first"
        );
        let mode = fs::metadata(&lock_at).expect("lock").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        drop(held);
        assert!(SocketLock::try_take(&lock_at).expect("opens").is_some());

        let at = base.join("local.sock");
        let first = Listener::bind(&at).expect("binds");
        let first_id = first.id().expect("known");
        drop(first);
        fs::remove_file(&at).expect("gone");
        let second = Listener::bind(&at).expect("binds again");
        remove_socket(&at, first_id);
        assert!(at.exists(), "the newer daemon's socket stays");
        remove_socket(&at, second.id().expect("known"));
        assert!(!at.exists(), "its own goes");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn a_folder_of_ours_with_any_mode_is_tightened_and_a_file_in_its_place_is_refused() {
        let base = std::env::temp_dir().join(format!("temor-remote-dir-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let dir = base.join("sockets");
        ensure_private_dir(&dir).expect("made");
        for mode in [0o300, 0o000, 0o755, 0o700] {
            fs::set_permissions(&dir, fs::Permissions::from_mode(mode)).expect("chmod");
            ensure_private_dir(&dir).unwrap_or_else(|err| panic!("{mode:o}: {err}"));
            let now = fs::metadata(&dir).expect("dir").permissions().mode() & 0o777;
            assert_eq!(now, 0o700, "from {mode:o}");
        }
        let file = base.join("a-file");
        fs::write(&file, "x").expect("file");
        assert!(ensure_private_dir(&file).is_err());
        let _ = fs::remove_dir_all(&base);
    }
}
