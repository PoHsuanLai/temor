//! Fixtures the tests build files and repositories from.

use std::path::Path;

/// A recognisable, non-repeating pattern of `len` bytes, for a file whose parts must land in
/// order.
pub fn pattern(len: u32) -> Vec<u8> {
    (0..len)
        .map(|n| (n % 251) as u8 ^ (n / 4093) as u8)
        .collect()
}

/// Runs git in `dir` as a test user, with no config of the person's.
pub fn git(dir: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}
