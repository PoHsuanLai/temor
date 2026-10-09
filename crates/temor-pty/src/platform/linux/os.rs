//! `Linux`: process facts from `/proc`.

use std::path::{Path, PathBuf};

use crate::platform::Os;
use crate::pty::{ProcRoot, ProcessId};
use temor_shell::UserId;

/// This OS.
#[derive(Debug)]
pub(crate) struct Linux;

impl Os for Linux {
    fn process_name(proc: &ProcRoot, pid: ProcessId) -> Option<String> {
        let comm = std::fs::read_to_string(proc.0.join(pid.0.to_string()).join("comm")).ok()?;
        Some(comm.trim_end_matches('\n').to_owned())
    }

    fn process_cwd(proc: &ProcRoot, pid: ProcessId) -> Option<PathBuf> {
        std::fs::read_link(proc.0.join(pid.0.to_string()).join("cwd")).ok()
    }

    fn current_user() -> Option<UserId> {
        Some(UserId(rustix::process::getuid().as_raw()))
    }

    fn is_executable(path: &Path) -> bool {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
    }
}
