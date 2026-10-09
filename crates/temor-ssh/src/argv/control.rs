//! The private runtime directory and the ControlMaster options derived from it.

use std::path::{Component, Path, PathBuf};

use crate::error::SshError;

/// ssh's `%C` hash is 40 hex digits (SHA-1).
const HASH_LEN: usize = 40;
/// ssh writes a master's socket under a temporary name with a 17-byte suffix (`.` and 16
/// random characters) before it renames it into place.
const SUFFIX_LEN: usize = 17;
/// `sun_path` is 108 bytes including the terminating NUL.
const SOCKET_PATH_MAX: usize = 107;

/// The runtime directory temor owns (for example `$XDG_RUNTIME_DIR/temor`). Absolute, with
/// only characters that need no quoting inside an `ssh -o` value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDir(PathBuf);

impl RuntimeDir {
    /// Validates the path: absolute, UTF-8, `[A-Za-z0-9/._-]` only, no `..` component.
    pub fn new(path: &Path) -> Result<Self, SshError> {
        let text = path.to_str().unwrap_or_default();
        let safe_chars = !text.is_empty()
            && text
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-'));
        let no_parent = path
            .components()
            .all(|c| !matches!(c, Component::ParentDir));
        match path.is_absolute() && safe_chars && no_parent {
            true => Ok(Self(path.to_path_buf())),
            false => Err(SshError::UnsafeValue(path.display().to_string())),
        }
    }

    /// The path.
    pub fn path(&self) -> &Path {
        &self.0
    }
}

/// A positive number of seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Seconds(u32);

impl Seconds {
    /// `None` for zero.
    pub fn new(seconds: u32) -> Option<Self> {
        (seconds > 0).then_some(Self(seconds))
    }

    /// The number.
    pub fn get(self) -> u32 {
        self.0
    }
}

/// How long a master outlives its last session (`ControlPersist`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ControlPersist {
    /// Exit after this long with no session attached.
    Idle(Seconds),
    /// Stay until something exits it (`ControlPersist=yes`).
    Forever,
}

/// The shared-connection layout: `<runtime>/ssh/%C`, so one socket per host, where ssh's `%C`
/// hashes the local and remote names, the port and the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlLayout {
    runtime: RuntimeDir,
    persist: ControlPersist,
}

impl ControlLayout {
    /// Checks that the socket path, with ssh's suffix, fits a unix socket.
    pub fn new(runtime: RuntimeDir, persist: ControlPersist) -> Result<Self, SshError> {
        let len = socket_dir_of(&runtime).as_os_str().len() + 1 + HASH_LEN;
        match len + SUFFIX_LEN <= SOCKET_PATH_MAX {
            true => Ok(Self { runtime, persist }),
            false => Err(SshError::ControlPathTooLong {
                len: len + SUFFIX_LEN,
                limit: SOCKET_PATH_MAX,
            }),
        }
    }

    /// The directory that holds the sockets; create it with
    /// [`crate::ensure_private_dir`] before any ssh runs.
    pub fn socket_dir(&self) -> PathBuf {
        socket_dir_of(&self.runtime)
    }

    /// `ControlPath=<socket dir>/%C`.
    pub fn control_path_option(&self) -> String {
        format!("ControlPath={}/%C", self.socket_dir().display())
    }

    /// `ControlPersist=<seconds>` or `ControlPersist=yes`.
    pub fn control_persist_option(&self) -> String {
        match self.persist {
            ControlPersist::Idle(seconds) => format!("ControlPersist={}", seconds.get()),
            ControlPersist::Forever => "ControlPersist=yes".to_owned(),
        }
    }
}

fn socket_dir_of(runtime: &RuntimeDir) -> PathBuf {
    runtime.path().join("ssh")
}
