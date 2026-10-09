//! Remote files over the shared connection (PLAN §9b): fetch a file to a local copy, write it
//! back after the person saves, and ask the host for a file's modified time and size so a
//! change on the host is noticed first. The commands are built here; the edge trait
//! [`RemoteFiles`] runs them, and the pure edit machine that decides when lives in temor-core.

mod argv;
mod edge;
mod path;
#[cfg(test)]
mod tests;

pub use argv::{FileTools, fetch_argv, stat_argv, store_argv};
pub use edge::{CommandOutput, CommandRunner, RemoteFiles, SshRemoteFiles, StdRunner};
pub use path::{ByteCount, RemoteFilePath, RemoteState, RemoteStat, parse_stat};
