//! Linux: process facts from `/proc`. (The pseudo-terminal is `unix_pty`.)

mod os;
#[cfg(test)]
mod os_tests;

pub(crate) use os::Linux;
