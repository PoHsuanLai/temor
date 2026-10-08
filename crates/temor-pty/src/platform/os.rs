//! `Os`: what temor reads from the operating system beyond portable std.

use std::path::{Path, PathBuf};

use crate::pty::{ProcRoot, ProcessId};
use crate::shell::UserId;

/// The process and file facts temor reads from the OS for tab titles, the running row, shell
/// resolution and program lookup. Each OS module implements it; `Current` is this build's.
pub(crate) trait Os {
    /// The short name (`comm`) of process `pid`, when the OS can tell.
    fn process_name(proc: &ProcRoot, pid: ProcessId) -> Option<String>;
    /// The working directory of process `pid`, when the OS can tell.
    fn process_cwd(proc: &ProcRoot, pid: ProcessId) -> Option<PathBuf>;
    /// The real user id of this process.
    fn current_user() -> Option<UserId>;
    /// Whether `path` is a file this user may run.
    fn is_executable(path: &Path) -> bool;
}
