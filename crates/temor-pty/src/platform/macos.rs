//! `MacOs`: stubs. temor has no pseudo-terminal here yet; `Pty::spawn` returns
//! `PtyError::Unsupported` (see `unsupported.rs`).

use std::path::{Path, PathBuf};

use crate::platform::Os;
use crate::pty::{ProcRoot, ProcessId};
use temor_shell::UserId;

/// This OS.
#[derive(Debug)]
pub(crate) struct MacOs;

impl Os for MacOs {
    // TODO(platform:macos): the foreground process's name.
    fn process_name(_proc: &ProcRoot, _pid: ProcessId) -> Option<String> {
        None
    }

    // TODO(platform:macos): the foreground process's working directory.
    fn process_cwd(_proc: &ProcRoot, _pid: ProcessId) -> Option<PathBuf> {
        None
    }

    // TODO(platform:macos): the last command of a foreground pipeline.
    fn pipeline_last(_proc: &ProcRoot, _shell: ProcessId, _group: ProcessId) -> Option<ProcessId> {
        None
    }

    // TODO(platform:macos): the current user, for the passwd lookup.
    fn current_user() -> Option<UserId> {
        None
    }

    // TODO(platform:macos): the executable check; any file counts for now.
    fn is_executable(path: &Path) -> bool {
        path.is_file()
    }
}
