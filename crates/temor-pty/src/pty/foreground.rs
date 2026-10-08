//! Describing a process group for tab titles and the running row.

use crate::platform::{Current, Os};
use crate::pty::{Foreground, ProcRoot, ProcessId};

impl Foreground {
    /// Process group `group` with its leader's name and working directory, as far as the OS
    /// can tell (`/proc` on Linux; `None` elsewhere for now).
    pub fn of_group(group: ProcessId, proc: &ProcRoot) -> Self {
        Self {
            group,
            name: <Current as Os>::process_name(proc, group),
            cwd: <Current as Os>::process_cwd(proc, group),
        }
    }
}
