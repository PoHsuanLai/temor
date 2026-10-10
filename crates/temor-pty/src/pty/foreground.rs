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

    /// Like [`Foreground::of_group`], but named for the last command of a pipeline: when the
    /// group is a job of `shell` and the OS can list the shell's children, the name is that of
    /// the group's newest child (`b` in `a | b`), not of its leader (`a`). The directory is the
    /// leader's.
    pub fn of_pipeline(group: ProcessId, shell: ProcessId, proc: &ProcRoot) -> Self {
        let mut found = Self::of_group(group, proc);
        if group != shell {
            let last = <Current as Os>::pipeline_last(proc, shell, group);
            let name = last.and_then(|last| <Current as Os>::process_name(proc, last));
            found.name = name.or(found.name);
        }
        found
    }
}
