//! Shared helpers for the window tests: a scratch directory and the tools they need.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use temor_pty::SessionLauncher;
use temor_remote::{Clock, DEFAULT_IDLE, Daemon, DaemonConfig, ManualClock};
pub use temor_testkit::ScratchRoot;
use temor_testkit::{programs_or_skip, skip};
use temor_wire::hello::HelperVersion;

/// How long a test waits for a real shell or a large transfer before it gives up. The wait ends
/// as soon as the condition holds, so a generous bound costs nothing when the machine is idle.
pub const SLOW_WAIT: Duration = Duration::from_secs(60);

static HEAVY: Mutex<()> = Mutex::new(());

/// Held by the tests that start several real shells or move megabytes through one, so a parallel
/// run does not start them all at once and starve each other past their deadlines. The other
/// tests still run beside them.
pub fn heavy() -> MutexGuard<'static, ()> {
    // A test that failed while holding it leaves nothing half-done for the next.
    HEAVY.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Where tests put their files: under the build directory, never under the real HOME.
pub fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("home")).expect("scratch home");
    dir
}

/// A scratch root for a test whose shared connection keeps its sockets in `<root>/<runtime>`.
/// It goes under the temporary folder when the sockets fit there, else under `/tmp`, so a long
/// `TMPDIR` cannot turn the test into a timeout. Panics when even `/tmp` is too long for them.
pub fn socket_root(name: &str, runtime: &str) -> ScratchRoot {
    ScratchRoot::for_control_sockets(name, runtime)
}

/// Where `scratch(name)` put its files, without making them again.
pub fn scratch_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name)
}

/// Whether the tools these tests drive are present: bash, util-linux `setsid`, and `/proc`.
/// Where one is missing the test returns early (with a note), or fails under
/// `TEMOR_REQUIRE_TOOLS=1`.
pub fn tools_present() -> bool {
    if programs_or_skip(&["bash", "setsid"], "temor-ui").is_none() {
        return false;
    }
    let proc = std::path::Path::new("/proc/self").exists();
    if !proc {
        skip("/proc", "temor-ui");
    }
    proc
}

/// Util-linux `setsid`; the tests that need it have checked [`tools_present`].
pub fn setsid_path() -> PathBuf {
    temor_testkit::find_program("setsid").expect("util-linux setsid is installed")
}

/// A helper in this process: a daemon over the Space folder `home`, its sockets in `dir/run`,
/// starting sessions in `shell`.
pub fn helper_daemon(
    helper: HelperVersion,
    dir: &std::path::Path,
    home: &std::path::Path,
    shell: PathBuf,
) -> std::sync::Arc<Daemon> {
    Daemon::new(DaemonConfig {
        helper,
        launcher: SessionLauncher::Setsid(setsid_path()),
        default_shell: shell,
        runtime_dir: dir.join("run"),
        home: Some(home.to_path_buf()),
        file_roots: vec![home.to_path_buf()],
        proc_root: "/proc".into(),
        idle: DEFAULT_IDLE,
        clock: std::sync::Arc::new(ManualClock::new()) as std::sync::Arc<dyn Clock>,
    })
}

/// Where bash is, from the standard places (the tests skip when it is not there).
pub fn bash_path() -> PathBuf {
    temor_pty::find_program("bash", std::ffi::OsStr::new("/usr/bin:/bin"))
        .unwrap_or_else(|| PathBuf::from("/usr/bin/bash"))
}
