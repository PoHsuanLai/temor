//! Polling until a deadline, and the small checks tests make while they wait.

use std::fmt::Display;
use std::path::Path;
use std::time::{Duration, Instant};

/// For a real shell or a large transfer. The wait ends as soon as the condition holds, so a
/// generous bound costs nothing on an idle machine.
pub const SLOW: Duration = Duration::from_secs(60);

const STEP: Duration = Duration::from_millis(10);

/// Runs `check` every few milliseconds until it is true; panics naming `what` after `timeout`.
pub fn eventually(what: &str, timeout: Duration, mut check: impl FnMut() -> bool) {
    eventually_some(what, timeout, || check().then_some(()));
}

/// Runs `check` every few milliseconds until it gives a value, and returns it; panics naming
/// `what` after `timeout`.
pub fn eventually_some<T>(
    what: &str,
    timeout: Duration,
    mut check: impl FnMut() -> Option<T>,
) -> T {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(value) = check() {
            return value;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(STEP);
    }
}

/// The process id a program wrote to `file` (a number and a newline), once it has.
pub fn pid_from_file(file: &Path, timeout: Duration) -> u32 {
    eventually_some(
        &format!("{} to hold a pid", file.display()),
        timeout,
        || std::fs::read_to_string(file).ok()?.trim().parse().ok(),
    )
}

/// Whether `/proc/<pid>` exists: the process is running or is a zombie nobody has reaped.
pub fn proc_alive(pid: impl Display) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

/// The offset of the first `needle` in `haystack`.
pub fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Whether `haystack` holds the bytes of `needle`.
pub fn contains(haystack: &[u8], needle: &str) -> bool {
    find(haystack, needle.as_bytes()).is_some()
}
