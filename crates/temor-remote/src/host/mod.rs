//! The helper's own edges: where the socket lives, who the helper is, the daemon's main loop and
//! the stdio proxy that `ssh host temor-remote` runs. Thin on purpose: the daemon is
//! [`crate::Daemon`], and this only wires it to the machine.

mod daemon_main;
mod env;
mod identity;
mod paths;
mod stdio_main;

pub use daemon_main::run_daemon;
pub use env::HostEnv;
pub use identity::hash_executable;
pub use paths::{runtime_dir, socket_path};
pub use stdio_main::run_stdio;
