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

    fn pipeline_last(proc: &ProcRoot, shell: ProcessId, group: ProcessId) -> Option<ProcessId> {
        let children = std::fs::read_to_string(
            proc.0
                .join(shell.0.to_string())
                .join("task")
                .join(shell.0.to_string())
                .join("children"),
        )
        .ok()?;
        children
            .split_whitespace()
            .filter_map(|word| word.parse::<i32>().ok().map(ProcessId))
            .filter_map(|child| {
                let (child_group, start) = stat_group_and_start(proc, child)?;
                (child_group == group).then_some((start, child))
            })
            // Started last, then the higher pid: pids wrap, start times do not.
            .max()
            .map(|(_, child)| child)
    }

    fn current_user() -> Option<UserId> {
        Some(UserId(rustix::process::getuid().as_raw()))
    }

    fn login_shell(uid: UserId) -> Option<PathBuf> {
        // getpwuid_r through NSS, so accounts from LDAP, sssd or systemd-homed are found too.
        let user = nix::unistd::User::from_uid(nix::unistd::Uid::from_raw(uid.0)).ok()??;
        (!user.shell.as_os_str().is_empty()).then_some(user.shell)
    }

    fn is_executable(path: &Path) -> bool {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
    }
}

/// The process group and start time (clock ticks since boot) of `pid`, from `stat`: the third
/// and twentieth fields after the command name, which may hold spaces and parentheses, so the
/// fields are counted from its last `)`. A start time that is missing reads as 0.
fn stat_group_and_start(proc: &ProcRoot, pid: ProcessId) -> Option<(ProcessId, u64)> {
    let stat = std::fs::read_to_string(proc.0.join(pid.0.to_string()).join("stat")).ok()?;
    let after = &stat[stat.rfind(')')? + 1..];
    let mut fields = after.split_whitespace();
    let group = fields.nth(2)?.parse::<i32>().ok().map(ProcessId)?;
    let start = fields.nth(16).and_then(|f| f.parse::<u64>().ok());
    Some((group, start.unwrap_or(0)))
}
