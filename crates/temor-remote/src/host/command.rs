//! The words after `temor-remote` on the command line: the helper's own subcommands.

/// The end of the ssh channel (also what no word means).
pub const STDIO: &str = "stdio";
/// The session daemon.
pub const DAEMON: &str = "daemon";
/// After [`DAEMON`]: the person's own daemon on this computer, not one for an ssh host.
pub const LOCAL: &str = "local";
/// How the daemon starts a session's program on its terminal.
pub const LEADER: &str = "leader";
/// Prints the helper's identity.
pub const VERSION: &str = "version";
