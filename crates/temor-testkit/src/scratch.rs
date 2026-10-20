//! Scratch folders: a HOME with XDG folders below it, and a root short enough for sockets.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

/// The longest path a unix socket takes (`sun_path` is 108 bytes with its final NUL).
pub const SOCKET_PATH_MAX: usize = 107;

/// A scratch HOME and the XDG folders below it, removed when dropped. Nothing in it is the
/// person's.
#[derive(Debug)]
pub struct Scratch {
    _root: TempDir,
    pub root: PathBuf,
    pub home: PathBuf,
    pub config: PathBuf,
    pub data: PathBuf,
    pub cache: PathBuf,
    pub state: PathBuf,
    pub runtime: PathBuf,
}

impl Scratch {
    /// New folders in the system temp folder.
    pub fn new() -> Self {
        let root = tempfile::Builder::new()
            .prefix("tmr-")
            .tempdir()
            .expect("a scratch folder");
        Self::in_root(root)
    }

    /// New folders under `base`, such as a build's `CARGO_TARGET_TMPDIR`.
    pub fn new_in(base: &Path) -> Self {
        let root = tempfile::Builder::new()
            .prefix("tmr-")
            .tempdir_in(base)
            .expect("a scratch folder");
        Self::in_root(root)
    }

    fn in_root(root: TempDir) -> Self {
        // Made real, as a shell sees it from getcwd (macOS's temp folder is under /var, a link).
        let base = root.path().canonicalize().expect("the scratch path");
        let home = base.join("home");
        let scratch = Self {
            config: home.join(".config"),
            data: home.join(".local/share"),
            cache: home.join(".cache"),
            state: home.join(".local/state"),
            runtime: base.join("run"),
            home,
            root: base,
            _root: root,
        };
        for folder in scratch.folders() {
            std::fs::create_dir_all(folder).expect("a scratch folder");
        }
        // A runtime folder is private to its person, and programs check.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&scratch.runtime, std::fs::Permissions::from_mode(0o700))
                .expect("a private runtime folder");
        }
        scratch
    }

    fn folders(&self) -> [&PathBuf; 6] {
        [
            &self.home,
            &self.config,
            &self.data,
            &self.cache,
            &self.state,
            &self.runtime,
        ]
    }

    /// The variables that point a program at these folders and nowhere else.
    pub fn env(&self) -> Vec<(&'static str, OsString)> {
        [
            ("HOME", &self.home),
            ("XDG_CONFIG_HOME", &self.config),
            ("XDG_DATA_HOME", &self.data),
            ("XDG_CACHE_HOME", &self.cache),
            ("XDG_STATE_HOME", &self.state),
            ("XDG_RUNTIME_DIR", &self.runtime),
        ]
        .into_iter()
        .map(|(name, path)| (name, path.clone().into_os_string()))
        .collect()
    }

    /// Points `command` at these folders: clears the person's HOME and XDG variables first.
    pub fn apply<'a>(&self, command: &'a mut Command) -> &'a mut Command {
        for (name, value) in self.env() {
            command.env(name, value);
        }
        command
    }
}

impl Default for Scratch {
    fn default() -> Self {
        Self::new()
    }
}

/// A scratch folder with a name of the test's choosing, emptied when it is made and removed when
/// it is dropped, a failing test's included.
#[derive(Debug)]
pub struct ScratchRoot(PathBuf);

impl ScratchRoot {
    /// A root for a test whose sockets live below it, the longest at `longest_below` bytes after
    /// the root (a leading `/` counted). It goes in the system temp folder when the sockets fit
    /// there, else in `/tmp`, so a long `TMPDIR` cannot turn the test into a timeout. Panics when
    /// even `/tmp` is too long.
    pub fn for_sockets(name: &str, longest_below: usize) -> Self {
        let leaf = format!("tmr-{}-{name}", std::process::id());
        let fits =
            |base: &Path| base.join(&leaf).as_os_str().len() + longest_below <= SOCKET_PATH_MAX;
        let temp = std::env::temp_dir();
        let base = if fits(&temp) {
            temp
        } else {
            PathBuf::from("/tmp")
        };
        assert!(
            fits(&base),
            "the scratch root for {name} leaves no room for a socket path under {}",
            base.display()
        );
        let root = base.join(leaf);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a scratch root");
        Self(root)
    }
}

/// What a shared ssh connection adds below its runtime folder: `/ssh/`, ssh's 40-digit `%C`
/// hash, and the 17-byte suffix of the name a master listens under before it renames the socket.
const CONTROL_SOCKET_TAIL: usize = "/ssh/".len() + 40 + 17;

impl ScratchRoot {
    /// A root for a test whose shared ssh connection keeps its sockets in `<root>/<runtime>`
    /// (see [`ScratchRoot::for_sockets`]).
    pub fn for_control_sockets(name: &str, runtime: &str) -> Self {
        Self::for_sockets(name, 1 + runtime.len() + CONTROL_SOCKET_TAIL)
    }
}

impl std::ops::Deref for ScratchRoot {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
