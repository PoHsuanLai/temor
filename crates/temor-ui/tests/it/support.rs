//! Shared helpers for the window tests: a scratch directory and the tools they need.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

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

/// Where `scratch(name)` put its files, without making them again.
pub fn scratch_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name)
}

/// Whether the tools these tests drive are present: bash, util-linux `setsid`, and `/proc`.
/// Where one is missing the test returns early (with a note) instead of failing.
pub fn tools_present() -> bool {
    let path = std::ffi::OsStr::new("/usr/bin:/bin");
    let found = temor_pty::find_program("bash", path).is_some()
        && temor_pty::find_program("setsid", path).is_some()
        && std::path::Path::new("/proc/self").exists();
    if !found {
        // TEMOR_REQUIRE_TOOLS=1 (set by the lane's gate on a machine that has the tools) turns the
        // skip into a failure, so the tests cannot pass silently there.
        if std::env::var("TEMOR_REQUIRE_TOOLS").is_ok_and(|v| v == "1") {
            panic!("bash, setsid and /proc are required (TEMOR_REQUIRE_TOOLS=1)");
        }
        eprintln!(
            "SKIPPED: needs bash, setsid and /proc (set TEMOR_REQUIRE_TOOLS=1 to fail instead)"
        );
    }
    found
}

/// Where bash is, from the standard places (the tests skip when it is not there).
pub fn bash_path() -> PathBuf {
    temor_pty::find_program("bash", std::ffi::OsStr::new("/usr/bin:/bin"))
        .unwrap_or_else(|| PathBuf::from("/usr/bin/bash"))
}
