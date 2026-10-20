//! `MacOs`: process facts from `proc_pidinfo` through libproc, and the account
//! lookup through `getpwuid_r`.
//!
//! There are no `/proc` files here, so the `ProcRoot` the callers hold is not read.

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use libproc::bsd_info::BSDInfo;
use libproc::proc_pid::{PIDInfo, PidInfoFlavor, name, pidinfo};
use libproc::processes::{ProcFilter, pids_by_type};

use crate::platform::Os;
use crate::pty::{CommandLine, ProcRoot, ProcessId};
use temor_shell::UserId;

/// This OS.
#[derive(Debug)]
pub(crate) struct MacOs;

impl Os for MacOs {
    fn process_name(_proc: &ProcRoot, pid: ProcessId) -> Option<String> {
        name(pid.0).ok().filter(|name| !name.is_empty())
    }

    // TODO(platform:macos): the command line is `sysctl` `KERN_PROCARGS2`, which libproc does not
    // wrap and which needs `unsafe` or a sysctl crate; until then callers see no command line.
    fn process_command(_proc: &ProcRoot, _pid: ProcessId) -> Option<CommandLine> {
        None
    }

    fn process_cwd(_proc: &ProcRoot, pid: ProcessId) -> Option<PathBuf> {
        let info = pidinfo::<VnodePathInfo>(pid.0, 0).ok()?;
        let path = info.cdir_path.split(|b| *b == 0).next()?;
        (!path.is_empty()).then(|| PathBuf::from(OsStr::from_bytes(path)))
    }

    fn pipeline_last(_proc: &ProcRoot, shell: ProcessId, group: ProcessId) -> Option<ProcessId> {
        // Only the members of the group are looked at, not every process.
        pids_by_type(ProcFilter::ByProgramGroup {
            pgrpid: u32::try_from(group.0).ok()?,
        })
        .ok()?
        .into_iter()
        .filter_map(|pid| {
            let pid = i32::try_from(pid).ok()?;
            let info = pidinfo::<BSDInfo>(pid, 0).ok()?;
            (info.pbi_ppid == u32::try_from(shell.0).ok()?)
                // Started last, then the higher pid: pids wrap, start times do not.
                .then_some((info.pbi_start_tvsec, info.pbi_start_tvusec, pid))
        })
        .max()
        .map(|(_, _, pid)| ProcessId(pid))
    }

    fn current_user() -> Option<UserId> {
        Some(UserId(rustix::process::getuid().as_raw()))
    }

    fn login_shell(uid: UserId) -> Option<PathBuf> {
        let user = nix::unistd::User::from_uid(nix::unistd::Uid::from_raw(uid.0)).ok()??;
        (!user.shell.as_os_str().is_empty()).then_some(user.shell)
    }

    fn is_executable(path: &Path) -> bool {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
    }
}

/// `struct proc_vnodepathinfo` (`PROC_PIDVNODEPATHINFO`): the process's current and root
/// directories, each a 152-byte `vnode_info` and a `MAXPATHLEN` path. libproc defines no struct
/// for it; its `pidinfo` fills any `PIDInfo` that is all plain bytes, as this is.
#[repr(C)]
struct VnodePathInfo {
    cdir_info: [u8; 152],
    cdir_path: [u8; 1024],
    rdir: [u8; 152 + 1024],
}

impl PIDInfo for VnodePathInfo {
    fn flavor() -> PidInfoFlavor {
        PidInfoFlavor::VNodePathInfo
    }
}
