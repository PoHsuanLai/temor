//! The window's SSH (PLAN §9b, Tier A): where its programs and sockets are, the host list, and
//! the jobs that wait on a program or the network, each on a thread of its own.

pub mod env;
mod hosts_cache;
pub mod view;
pub mod work;
#[cfg(test)]
mod work_tests;

pub use env::{ArcRunner, SshEnv};

// The hosts and how they are named: the host list and a host's key.
pub use temor_ssh::{
    DirectHost, Fingerprint, HostAlias, HostKey, HostName, HostRow, KnownName, Presence,
    is_ipv6_literal, merge_hosts,
};
// The shared connection to a host and the command lines that use it.
pub use temor_ssh::{
    BootstrapOptions, BootstrapScript, ControlLayout, MasterProbe, bootstrap_remote_command,
    plain_session_argv, prepare_control_dirs, session_argv, socket_appeared,
};
// The shell on the host: its probe, the line that loads temor's integration there and what is
// said before it starts.
pub use temor_ssh::{
    CheckScan, CheckUrl, InnerLoadLine, InnerShell, LoadToken, PreShellEvent, ProbeReading,
    probe_line, prompt_after_answer, read_probe,
};
// A file on a host, and the words of the faults.
pub use temor_ssh::{ByteCount, Port, RemoteFilePath, RemoteStat, SshError, UnixSeconds};
