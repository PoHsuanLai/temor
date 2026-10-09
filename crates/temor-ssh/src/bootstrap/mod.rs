//! The bootstrap sent as the remote command (PLAN §9b): integration scripts, a terminfo
//! fallback and an OSC 7 cwd, all in a temp directory removed on exit.

mod base64;
mod files;
mod quote;
mod script;
#[cfg(test)]
mod tests;

pub use script::{
    BootstrapOptions, BootstrapScript, MAX_REMOTE_COMMAND_BYTES, TerminfoPolicy,
    bootstrap_remote_command, wrap_script,
};
