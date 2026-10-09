//! temor's SSH, Tier A (PLAN §9b, D17): nothing is installed on the host.
//!
//! This crate turns `~/.ssh/config` into host rows, builds the exact `ssh` argv for a shared
//! connection per host (ControlMaster), its control commands and port forwards, and produces
//! the POSIX bootstrap that is sent as the remote command: shell integration, a terminfo
//! fallback and an OSC 7 cwd. It derives a host's identity from its key fingerprint.
//!
//! The crate never runs `ssh` and never opens a network connection; the app does that with the
//! argv built here. Inputs (the config files, the runtime directory, `$HOME`) are injected, and
//! the library spawns no threads. The only edge is the private runtime directory, which lives in
//! the private `platform` module with TODO stubs for the other OSes.

mod argv;
mod bootstrap;
mod config;
mod error;
mod identity;
mod platform;

pub use argv::{
    BindAddress, ControlLayout, ControlPersist, Forward, ForwardSpec, RemoteCommand, RemoteHost,
    RuntimeDir, Seconds, cancel_forward_argv, check_argv, exit_argv, forward_argv, session_argv,
};
pub use bootstrap::{BootstrapScript, bootstrap_remote_command};
pub use config::{
    ConfigFiles, ConfigIssue, ConfigPaths, HostAlias, HostEntry, HostList, HostName, IdentityFile,
    IssueReason, LineNo, Port, ProxyJump, UserName, known_host_names, load_host_list,
};
pub use error::SshError;
pub use identity::{Fingerprint, HostId, ShownName, fingerprint_from_keygen};
pub use platform::ensure_private_dir;
