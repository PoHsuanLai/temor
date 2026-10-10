//! The bootstrap sent as the remote command (PLAN §9b): integration scripts, a terminfo
//! fallback and an OSC 7 cwd, all in a temp directory removed on exit.

mod base64;
mod files;
mod inner;
mod inner_probe;
mod inner_shell;
#[cfg(test)]
mod inner_tests;
mod quote;
mod script;
#[cfg(test)]
mod tests;

pub use inner::{InnerLoadLine, LoadToken, MAX_INNER_LINE_BYTES, MAX_TTY_LINE_BYTES};
pub use inner_probe::{ProbeReading, probe_line, read_probe};
pub use inner_shell::InnerShell;
pub use script::{
    BootstrapOptions, BootstrapScript, MAX_REMOTE_COMMAND_BYTES, TerminfoPolicy,
    bootstrap_remote_command, wrap_script,
};
