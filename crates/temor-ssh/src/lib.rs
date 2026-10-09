//! temor's SSH, Tier A (PLAN §9b, D17): nothing is installed on the host.
//!
//! This crate turns `~/.ssh/config` into host rows, builds the exact `ssh` argv for a shared
//! connection per host (ControlMaster), its control commands and port forwards, and produces
//! the POSIX bootstrap that is sent as the remote command: shell integration, a terminfo
//! fallback and an OSC 7 cwd. It derives a host's identity from its key fingerprint.
//!
//! The crate opens no network connection itself. The app runs `ssh` with the argv built here;
//! the one exception is [`SshRemoteFiles`], which runs `ssh` and `scp` through an injected
//! [`CommandRunner`] when the app asks it to move a file. Inputs (the config files, the runtime
//! directory, `$HOME`) are injected, and the library spawns no threads. Its OS edge is the
//! private runtime directory, in the private `platform` module with TODO stubs for the other
//! OSes.
//!
//! Beyond the host list it also merges in the person's tailnet machines ([`merge_hosts`]),
//! recognises Tailscale's browser check ([`CheckScan`]), and reads and writes remote files
//! through the shared connection ([`RemoteFiles`]).
//!
//! # Example
//!
//! The argv that asks whether a shared connection is up. The crate builds it; the app runs `ssh`:
//!
//! ```
//! use std::path::Path;
//! use temor_ssh::{
//!     ControlLayout, ControlPersist, HostAlias, RuntimeDir, Seconds, SshError, check_argv,
//! };
//!
//! # fn main() -> Result<(), SshError> {
//! let runtime = RuntimeDir::new(Path::new("/run/user/1000/temor"))?;
//! let persist = ControlPersist::Idle(Seconds::new(600).expect("600 is a positive number"));
//! let layout = ControlLayout::new(runtime, persist)?;
//! let argv = check_argv(&HostAlias::new("prod")?, &layout);
//! assert!(argv.iter().any(|arg| arg == "check"));
//! assert_eq!(argv.last().map(String::as_str), Some("prod"));
//! # Ok(())
//! # }
//! ```

mod argv;
mod bootstrap;
mod check_mode;
mod config;
mod error;
mod files;
mod hosts;
mod identity;
mod master;
mod platform;
mod tailnet;

/// In-memory config fixtures for tests; built with the `testing` feature.
#[cfg(feature = "testing")]
pub use config::support as testing;

pub use argv::{
    BindAddress, ControlLayout, ControlPersist, Forward, ForwardSpec, RemoteCommand, RemoteHost,
    RuntimeDir, Seconds, cancel_forward_argv, check_argv, exit_argv, forward_argv, session_argv,
};
pub use bootstrap::{
    BootstrapOptions, BootstrapScript, MAX_REMOTE_COMMAND_BYTES, TerminfoPolicy,
    bootstrap_remote_command, wrap_script,
};
pub use check_mode::{CheckScan, CheckUrl, PreShellEvent};
pub use config::{
    ConfigFiles, ConfigIssue, ConfigPaths, FsConfigFiles, HostAlias, HostEntry, HostList, HostName,
    IdentityFile, IssueReason, LineNo, Port, ProxyJump, UserName, known_host_names, load_host_list,
};
pub use error::SshError;
pub use files::{
    ByteCount, CommandOutput, CommandRunner, FileTools, RemoteFilePath, RemoteFiles, RemoteStat,
    RemoteState, SshRemoteFiles, StdRunner, fetch_argv, parse_stat, stat_argv, store_argv,
};
pub use hosts::{HostGroup, HostRow, merge_hosts};
pub use identity::{Fingerprint, HostId, HostKey, NodeId, ShownName, fingerprint_from_keygen};
pub use master::{MasterProbe, prepare_control_dirs, probe_master};
pub use platform::ensure_private_dir;
pub use tailnet::{DirectHost, Presence, TailnetHost, TailnetOwner, TailscaleSsh, UnixSeconds};
