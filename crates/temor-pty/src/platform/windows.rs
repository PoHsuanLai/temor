//! `Windows`: stubs. temor has no pseudo-terminal here yet; `Pty::spawn` returns
//! `PtyError::Unsupported` (see `unsupported.rs`).
//! Every OS other than Linux and macOS uses these stubs too.

use std::path::{Path, PathBuf};

use crate::platform::Os;
use crate::pty::{ProcRoot, ProcessId};
use temor_shell::UserId;

/// This OS.
#[derive(Debug)]
pub(crate) struct Windows;

impl Os for Windows {
    // TODO(platform:windows): the foreground process's name.
    fn process_name(_proc: &ProcRoot, _pid: ProcessId) -> Option<String> {
        None
    }

    // TODO(platform:windows): the foreground process's working directory.
    fn process_cwd(_proc: &ProcRoot, _pid: ProcessId) -> Option<PathBuf> {
        None
    }

    // TODO(platform:windows): the current user, for the passwd lookup.
    fn current_user() -> Option<UserId> {
        None
    }

    // TODO(platform:windows): the executable check; any file counts for now.
    fn is_executable(path: &Path) -> bool {
        path.is_file()
    }
}
